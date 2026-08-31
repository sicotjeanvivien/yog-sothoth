//! JSON-RPC adapter: `getTransaction` response → [`TransactionView`].
//!
//! This is the **only** module of `core` that names the JSON-RPC transport.
//! Everything downstream — the trait, the dispatcher, the per-protocol
//! extractors, the Anchor decoder — sees a [`TransactionView`] and cannot tell
//! which source filled it. A second source (Yellowstone gRPC) adds a sibling
//! adapter, not a second code path through extraction.
//!
//! It also re-exports the transport types the ingestion binary needs, because
//! the encoding and this adapter are one contract: the fetcher must ask for
//! `UiTransactionEncoding::JsonParsed`, since [`from_rpc`] reads the
//! `PartiallyDecoded` inner instructions that only that encoding produces.
//! Declaring them anywhere else would let the two drift apart.

use std::str::FromStr;

use bs58::decode as bs58_decode;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

pub use solana_transaction_status_client_types::{
    EncodedConfirmedTransactionWithStatusMeta, EncodedTransaction, UiInstruction,
    UiParsedInstruction, UiTransactionEncoding, option_serializer::OptionSerializer,
};

use crate::application::extraction::{InnerInstructionPayload, TransactionView};
use crate::domain::TransactionPosition;
use crate::{CoreError, CoreResult};

/// Build the neutral view from a `getTransaction` response.
///
/// Fails only on a transaction-level malformation — an encoding that carries no
/// signature, an unparsable signature, or a missing `blockTime`. A transaction
/// with no inner instructions is not a failure: it yields an empty payload list
/// and extraction reports "nothing to record".
pub fn from_rpc(tx: &EncodedConfirmedTransactionWithStatusMeta) -> CoreResult<TransactionView> {
    Ok(TransactionView {
        position: TransactionPosition {
            signature: extract_signature(tx)?,
            timestamp: extract_timestamp(tx)?,
            // Read straight off the envelope: unlike the two above they need no
            // parsing, and `transaction_index` is `None` on this path —
            // `getTransaction` does not return it (see `EventPosition`).
            slot: tx.slot,
            transaction_index: tx.transaction_index,
        },
        inner_instructions: extract_inner_instructions(tx),
    })
}

impl TransactionView {
    /// Convenience form of [`from_rpc`].
    pub fn from_rpc(tx: &EncodedConfirmedTransactionWithStatusMeta) -> CoreResult<Self> {
        from_rpc(tx)
    }
}

/// Extract the first transaction signature.
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

/// Flatten `meta.innerInstructions` into the ordered payload list the view owes
/// its readers.
///
/// Groups are sorted by the outer instruction they belong to, never left in
/// whatever order the RPC serialized them — the order is the `event_index`
/// contract documented on [`TransactionView::inner_instructions`].
///
/// Instructions the RPC returns in another shape than `PartiallyDecoded` are
/// dropped, and so are those whose `data` is not valid base58. Both are
/// representations this adapter cannot turn into bytes, not judgements about
/// what the payload means: `Parsed` proper is produced for instructions the
/// RPC's own parser recognizes (SPL Token and friends), which Anchor self-CPI
/// instructions never are. Narrowing this any further renumbers stored events —
/// see [`InnerInstructionPayload`].
fn extract_inner_instructions(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
) -> Vec<InnerInstructionPayload> {
    let Some(meta) = tx.transaction.meta.as_ref() else {
        return Vec::new();
    };

    let OptionSerializer::Some(inner_groups) = &meta.inner_instructions else {
        return Vec::new();
    };

    let mut groups: Vec<_> = inner_groups.iter().collect();
    groups.sort_by_key(|g| g.index);

    groups
        .into_iter()
        .flat_map(|group| group.instructions.iter())
        .filter_map(to_payload)
        .collect()
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
#[path = "rpc_tests.rs"]
mod tests;
