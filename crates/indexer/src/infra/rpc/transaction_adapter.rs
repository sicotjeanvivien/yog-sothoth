//! JSON-RPC adapter: `getTransaction` response → [`OnChainTransaction`].
//! ⚠️ One contract with the encoding: `TransactionFetcher` must ask for `JsonParsed`, the only one
//! that yields the `PartiallyDecoded` inner instructions [`from_rpc`] reads.

use std::str::FromStr;

use bs58::decode as bs58_decode;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

use crate::infra::TransactionAdapterError;
use crate::infra::refusal::{self, Gap};
pub(crate) use solana_transaction_status_client_types::{
    EncodedConfirmedTransactionWithStatusMeta, UiTransactionEncoding,
};
use solana_transaction_status_client_types::{
    EncodedTransaction, UiInstruction, UiParsedInstruction, option_serializer::OptionSerializer,
};
use yog_core::application::extraction::{InnerInstructionPayload, OnChainTransaction};
use yog_core::domain::{Protocol, TransactionPosition};

/// Build the neutral transaction from a `getTransaction` response. Refusals, all counted under
/// `reason="adapt"`: no or bad `signatures[0]`, a non-`Json` encoding, `blockTime` absent or out of
/// range, no `meta`, no `innerInstructions`, an unreadable inner instruction of a program we may
/// index. An empty `innerInstructions` is ordinary traffic.
pub(crate) fn from_rpc(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
) -> Result<OnChainTransaction, TransactionAdapterError> {
    let signature = extract_signature(tx)?;

    Ok(OnChainTransaction {
        position: TransactionPosition {
            signature,
            timestamp: extract_timestamp(tx)?,
            slot: tx.slot,
            // Optional, and absent on the provider in use; see `EventPosition`.
            transaction_index: tx.transaction_index,
        },
        inner_instructions: extract_inner_instructions(tx, &signature)?,
    })
}

/// Extract `signatures[0]`, the transaction id: what `getTransaction` is queried by.
/// ⚠️ The others are co-signers; keying rows on one would file them where no lookup finds them.
fn extract_signature(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
) -> Result<Signature, TransactionAdapterError> {
    match &tx.transaction.transaction {
        EncodedTransaction::Json(ui_tx) => {
            let sig_str =
                ui_tx
                    .signatures
                    .first()
                    .ok_or_else(|| TransactionAdapterError::MissingField {
                        signature: String::new(),
                        field: "signatures".to_string(),
                    })?;

            Signature::from_str(sig_str).map_err(|e| TransactionAdapterError::Parse {
                signature: String::new(),
                reason: format!("invalid signature {sig_str}: {e}"),
            })
        }
        _ => Err(TransactionAdapterError::Parse {
            signature: String::new(),
            reason: "unexpected transaction encoding".to_string(),
        }),
    }
}

/// Extract the block timestamp as UTC.
fn extract_timestamp(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
) -> Result<DateTime<Utc>, TransactionAdapterError> {
    let block_time = tx
        .block_time
        .ok_or_else(|| TransactionAdapterError::MissingField {
            signature: String::new(),
            field: "blockTime".to_string(),
        })?;

    DateTime::from_timestamp(block_time, 0).ok_or_else(|| TransactionAdapterError::Parse {
        signature: String::new(),
        reason: format!("invalid timestamp: {block_time}"),
    })
}

/// Flatten `meta.innerInstructions` in outer-instruction order, the `event_index` contract of
/// [`OnChainTransaction::inner_instructions`]; drops what [`to_payload`] sets aside.
/// ⚠️ Narrowing that filter further renumbers stored events, see [`InnerInstructionPayload`].
fn extract_inner_instructions(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    signature: &Signature,
) -> Result<Vec<InnerInstructionPayload>, TransactionAdapterError> {
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

    groups
        .into_iter()
        .flat_map(|group| group.instructions.iter())
        .filter_map(|ix| to_payload(ix, signature).transpose())
        .collect()
}

/// Turn one RPC inner instruction into a neutral payload, `None` for one not `PartiallyDecoded`.
/// ⚠️ Unreadable, it refuses the transaction when it may be an indexed program's: dropping
/// it would renumber the rest. A foreign program's unreadable data is set aside.
fn to_payload(
    ix: &UiInstruction,
    signature: &Signature,
) -> Result<Option<InnerInstructionPayload>, TransactionAdapterError> {
    let UiInstruction::Parsed(UiParsedInstruction::PartiallyDecoded(p)) = ix else {
        return Ok(None);
    };
    let unreadable = |reason: String| TransactionAdapterError::Parse {
        signature: signature.to_string(),
        reason,
    };

    let program_id = Pubkey::from_str(&p.program_id).map_err(|e| {
        unreadable(format!(
            "inner instruction program id {}: {e}",
            p.program_id
        ))
    })?;
    match bs58_decode(&p.data).into_vec() {
        Ok(data) => Ok(Some(InnerInstructionPayload { program_id, data })),
        Err(_) if Protocol::from_program_id(&program_id).is_none() => Ok(None),
        Err(e) => Err(unreadable(format!(
            "inner instruction data of {program_id} is not base58: {e}"
        ))),
    }
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
