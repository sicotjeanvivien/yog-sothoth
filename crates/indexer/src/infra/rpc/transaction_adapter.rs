//! JSON-RPC adapter: `getTransaction` response → [`OnChainTransaction`].
//! ⚠️ One contract with the encoding: `TransactionFetcher` must ask for `JsonParsed`, the only one
//! that yields the `PartiallyDecoded` inner instructions [`from_rpc`] reads.

use std::str::FromStr;

use bs58::decode as bs58_decode;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

use crate::infra::refusal::{self, Gap};
pub(crate) use solana_transaction_status_client_types::{
    EncodedConfirmedTransactionWithStatusMeta, UiTransactionEncoding,
};
use solana_transaction_status_client_types::{
    EncodedTransaction, UiInstruction, UiParsedInstruction, option_serializer::OptionSerializer,
};
use yog_core::application::extraction::{InnerInstructionPayload, OnChainTransaction};
use yog_core::domain::TransactionPosition;
use yog_core::{CoreError, CoreResult};

/// Build the neutral transaction from a `getTransaction` response. Refusals, all counted under
/// `reason="adapt"`: no or bad `signatures[0]`, a non-`Json` encoding, `blockTime` absent or out of
/// range, no `meta`, no `innerInstructions`. An empty `innerInstructions` is ordinary traffic.
pub(crate) fn from_rpc(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
) -> CoreResult<OnChainTransaction> {
    let signature = extract_signature(tx)?;

    Ok(OnChainTransaction {
        position: TransactionPosition {
            signature,
            timestamp: extract_timestamp(tx)?,
            // `transaction_index` is optional, and absent on the provider in use; see `EventPosition`.
            slot: tx.slot,
            transaction_index: tx.transaction_index,
        },
        inner_instructions: extract_inner_instructions(tx, &signature)?,
    })
}

/// Extract `signatures[0]`, the transaction id: what `getTransaction` is queried by.
/// ⚠️ The others are co-signers; keying rows on one would file them where no lookup finds them.
fn extract_signature(tx: &EncodedConfirmedTransactionWithStatusMeta) -> CoreResult<Signature> {
    match &tx.transaction.transaction {
        EncodedTransaction::Json(ui_tx) => {
            let sig_str = ui_tx
                .signatures
                .first()
                .ok_or_else(|| CoreError::MissingField {
                    signature: String::new(),
                    field: "signatures".to_string(),
                })?;

            Signature::from_str(sig_str).map_err(|e| CoreError::ParseError {
                signature: String::new(),
                reason: format!("invalid signature {sig_str}: {e}"),
            })
        }
        _ => Err(CoreError::ParseError {
            signature: String::new(),
            reason: "unexpected transaction encoding".to_string(),
        }),
    }
}

/// Extract the block timestamp as UTC.
fn extract_timestamp(tx: &EncodedConfirmedTransactionWithStatusMeta) -> CoreResult<DateTime<Utc>> {
    let block_time = tx.block_time.ok_or_else(|| CoreError::MissingField {
        signature: String::new(),
        field: "blockTime".to_string(),
    })?;

    DateTime::from_timestamp(block_time, 0).ok_or_else(|| CoreError::ParseError {
        signature: String::new(),
        reason: format!("invalid timestamp: {block_time}"),
    })
}

/// Flatten `meta.innerInstructions` in outer-instruction order, the `event_index` contract of
/// [`OnChainTransaction::inner_instructions`]; drops what is not `PartiallyDecoded` or not base58.
/// ⚠️ Narrowing that filter further renumbers stored events, see [`InnerInstructionPayload`].
fn extract_inner_instructions(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    signature: &Signature,
) -> CoreResult<Vec<InnerInstructionPayload>> {
    // ⚠️ No `meta` means "not captured", like the field below: an empty list would record a
    // transaction full of events as "nothing to record". Refusing counts the loss, not recovers it.
    let Some(meta) = tx.transaction.meta.as_ref() else {
        return Err(refusal::refuse(Gap::Meta, signature));
    };

    // `OptionSerializer::Skip` is serialize-only: an absent key and `null` both land on `None`,
    // "not carried"; "none" arrives as `Some([])` and flows on.
    let OptionSerializer::Some(inner_groups) = &meta.inner_instructions else {
        return Err(refusal::refuse(Gap::InnerInstructions, signature));
    };

    let mut groups: Vec<_> = inner_groups.iter().collect();
    groups.sort_by_key(|g| g.index);

    Ok(groups
        .into_iter()
        .flat_map(|group| group.instructions.iter())
        .filter_map(to_payload)
        .collect())
}

/// Turn one RPC inner instruction into a neutral payload, or `None` when this
/// adapter cannot represent it.
fn to_payload(ix: &UiInstruction) -> Option<InnerInstructionPayload> {
    let UiInstruction::Parsed(UiParsedInstruction::PartiallyDecoded(p)) = ix else {
        return None;
    };

    Some(InnerInstructionPayload {
        program_id: Pubkey::from_str(&p.program_id).ok()?,
        data: bs58_decode(&p.data).into_vec().ok()?,
    })
}

#[cfg(test)]
#[path = "tests/transaction_adapter_tests.rs"]
mod tests;

// ⚠️ Unit tests, not `tests/` targets: this crate is a binary, and an integration target could not
// reach the `pub(crate)` adapter.
#[cfg(test)]
#[path = "tests/fixture_pipeline_tests.rs"]
mod fixture_pipeline_tests;

#[cfg(test)]
#[path = "tests/extraction_oracle_tests.rs"]
mod extraction_oracle_tests;
