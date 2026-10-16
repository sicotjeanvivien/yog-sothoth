//! Yellowstone protobuf adapter: `SubscribeUpdateTransaction` → [`OnChainTransaction`].
//! ⚠️ Its tests build input by hand, with the understanding the code reads it with: green means
//! self-consistent, not right — no mainnet protobuf fixture exists.
//! ⚠️ Neither `meta.err` nor `is_vote` is read: `failed: Some(false)` and `vote: Some(false)` in
//! [`super::subscription::build_request`] are what keep reverted transactions and votes out.

use crate::infra::refusal::{self, Gap};
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use solana_signature::{SIGNATURE_BYTES, Signature};
use yellowstone_grpc_proto::prelude::{
    InnerInstruction, SubscribeUpdateTransaction, SubscribeUpdateTransactionInfo,
    TransactionStatusMeta,
};
use yog_core::application::extraction::{InnerInstructionPayload, OnChainTransaction};
use yog_core::domain::TransactionPosition;
use yog_core::{CoreError, CoreResult};

/// Build the neutral transaction; `timestamp` is an argument because the update carries none.
/// Refuses: no `transaction`, a signature not `SIGNATURE_BYTES` long, an `index` over `u32`, no
/// `message`, no `meta` or `inner_instructions_none` set, an index outside the keys, a key not 32 bytes.
pub(crate) fn from_grpc(
    update: &SubscribeUpdateTransaction,
    timestamp: DateTime<Utc>,
) -> CoreResult<OnChainTransaction> {
    let info = update
        .transaction
        .as_ref()
        .ok_or_else(|| CoreError::MissingField {
            signature: String::new(),
            field: "transaction".to_string(),
        })?;

    let signature = extract_signature(info)?;

    Ok(OnChainTransaction {
        position: TransactionPosition {
            signature,
            timestamp,
            slot: update.slot,
            transaction_index: Some(extract_index(info, &signature)?),
        },
        inner_instructions: extract_inner_instructions(info, &signature)?,
    })
}

/// Read the transaction id from its raw bytes; the provider has already picked `signatures[0]`.
fn extract_signature(info: &SubscribeUpdateTransactionInfo) -> CoreResult<Signature> {
    Signature::try_from(info.signature.as_slice()).map_err(|_| CoreError::ParseError {
        signature: String::new(),
        // `SIGNATURE_BYTES`, not `size_of::<Signature>()`: the wire constant is what the message promises.
        reason: format!(
            "signature is {} bytes, expected {SIGNATURE_BYTES}",
            info.signature.len(),
        ),
    })
}

/// Narrow the transaction's position in its slot to the domain's `u32`.
/// ⚠️ An error, never a truncating cast: a wrapped index would order same-slot events wrongly, silently.
fn extract_index(info: &SubscribeUpdateTransactionInfo, signature: &Signature) -> CoreResult<u32> {
    u32::try_from(info.index).map_err(|_| CoreError::ParseError {
        signature: signature.to_string(),
        reason: format!("transaction index {} does not fit in u32", info.index),
    })
}

/// Flatten `meta.inner_instructions` in outer-instruction order, the `event_index` contract of
/// [`OnChainTransaction::inner_instructions`]; nothing is filtered, the program filter runs downstream.
fn extract_inner_instructions(
    info: &SubscribeUpdateTransactionInfo,
    signature: &Signature,
) -> CoreResult<Vec<InnerInstructionPayload>> {
    // ⚠️ No `meta` means "not captured", like the flag below: an empty list would record a
    // transaction full of events as "nothing to record".
    let Some(meta) = info.meta.as_ref() else {
        return Err(refusal::refuse(Gap::Meta, signature));
    };

    // ⚠️ `inner_instructions_none` means "not captured", not "none": refusing puts the
    // transaction on the counted skip-and-log path instead of losing its events.
    if meta.inner_instructions_none {
        return Err(refusal::refuse(Gap::InnerInstructions, signature));
    }

    let account_keys = account_key_segments(info, meta, signature)?;

    let mut groups: Vec<_> = meta.inner_instructions.iter().collect();
    groups.sort_by_key(|g| g.index);

    groups
        .into_iter()
        .flat_map(|group| group.instructions.iter())
        .map(|ix| to_payload(ix, &account_keys, signature))
        .collect()
}

/// The segments an instruction index resolves against, in the order `solana_message::AccountKeys::get`
/// walks them: static keys, loaded writable, loaded readonly.
/// ⚠️ A missing message is an error: zero static keys would shift every index onto a valid, wrong key.
fn account_key_segments<'a>(
    info: &'a SubscribeUpdateTransactionInfo,
    meta: &'a TransactionStatusMeta,
    signature: &Signature,
) -> CoreResult<[&'a [Vec<u8>]; 3]> {
    let static_keys = info
        .transaction
        .as_ref()
        .and_then(|tx| tx.message.as_ref())
        .map(|message| message.account_keys.as_slice())
        .ok_or_else(|| CoreError::MissingField {
            // Named: on the skip-and-log path this line is all an operator gets.
            signature: signature.to_string(),
            field: "transaction.message".to_string(),
        })?;

    Ok([
        static_keys,
        meta.loaded_writable_addresses.as_slice(),
        meta.loaded_readonly_addresses.as_slice(),
    ])
}

/// Resolve one `program_id_index` against the segments above.
/// ⚠️ Out of range is an error, not a skip: an absent payload would renumber every later `event_index`.
fn resolve_program_id(
    index: u32,
    segments: &[&[Vec<u8>]; 3],
    signature: &Signature,
) -> CoreResult<Pubkey> {
    let mut remaining = index as usize;

    for segment in segments {
        if remaining < segment.len() {
            return Pubkey::try_from(segment[remaining].as_slice()).map_err(|_| {
                CoreError::ParseError {
                    signature: signature.to_string(),
                    reason: format!(
                        "account key at index {index} is {} bytes, not a public key",
                        segment[remaining].len()
                    ),
                }
            });
        }
        remaining -= segment.len();
    }

    let total: usize = segments.iter().map(|s| s.len()).sum();
    Err(CoreError::ParseError {
        signature: signature.to_string(),
        reason: format!("program_id_index {index} is outside the {total} account keys"),
    })
}

/// Turn one protobuf inner instruction into a neutral payload.
fn to_payload(
    ix: &InnerInstruction,
    segments: &[&[Vec<u8>]; 3],
    signature: &Signature,
) -> CoreResult<InnerInstructionPayload> {
    Ok(InnerInstructionPayload {
        program_id: resolve_program_id(ix.program_id_index, segments, signature)?,
        data: ix.data.clone(),
    })
}

#[cfg(test)]
#[path = "tests/transaction_adapter_tests.rs"]
mod tests;
