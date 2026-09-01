//! JSON-RPC adapter: `getTransaction` response → [`OnChainTransaction`].
//!
//! One of the adapters that fill the neutral transaction `yog-core` extracts
//! from. It lives here, beside the fetcher whose response it reads, because
//! `core` has no business knowing who supplies it — a second source
//! (Yellowstone gRPC) becomes a sibling module, not a second code path through
//! extraction.
//!
//! The encoding and this adapter are **one contract**: `TransactionFetcher`
//! must ask for `UiTransactionEncoding::JsonParsed`, since [`from_rpc`] reads
//! the `PartiallyDecoded` inner instructions that only that encoding produces.
//! Keeping them in the same crate is what stops the two from drifting apart.
//!
//! What it owes the rest of the workspace is stated once, in
//! `yog_core::application::extraction::conformance`, and proven against a real
//! mainnet response by `the_reference_transaction_is_what_this_adapter_produces`
//! in this module's tests.

use std::str::FromStr;

use bs58::decode as bs58_decode;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

pub(crate) use solana_transaction_status_client_types::{
    EncodedConfirmedTransactionWithStatusMeta, UiTransactionEncoding,
};
use solana_transaction_status_client_types::{
    EncodedTransaction, UiInstruction, UiParsedInstruction, option_serializer::OptionSerializer,
};
use yog_core::application::extraction::{InnerInstructionPayload, OnChainTransaction};
use yog_core::domain::TransactionPosition;
use yog_core::{CoreError, CoreResult};

/// Build the transport-neutral transaction from a `getTransaction` response.
///
/// Fails only on a transaction-level malformation — an encoding that carries no
/// signature, an unparsable signature, or a missing `blockTime`. A transaction
/// with no inner instructions is not a failure: it yields an empty payload list
/// and extraction reports "nothing to record".
pub(crate) fn from_rpc(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
) -> CoreResult<OnChainTransaction> {
    Ok(OnChainTransaction {
        position: TransactionPosition {
            signature: extract_signature(tx)?,
            timestamp: extract_timestamp(tx)?,
            // Read straight off the envelope: unlike the two above they need
            // no parsing. `transaction_index` is optional in the response and
            // absent on the ingestion path in use — a property of the
            // provider, not of `getTransaction`; see `EventPosition`.
            slot: tx.slot,
            transaction_index: tx.transaction_index,
        },
        inner_instructions: extract_inner_instructions(tx),
    })
}

/// Extract the transaction's signature — `signatures[0]`, which **is** its id.
///
/// Not an arbitrary pick among several. A transaction carries one signature per
/// required signer, ordered to match the leading account keys, and
/// `account_keys[0]` is the fee payer — so `signatures[0]` is what Solana calls
/// the transaction id: what an explorer indexes, and the argument
/// `getTransaction` is queried by. Ten of the 27 mainnet fixtures in
/// `core/tests/fixtures/damm_v2/` carry two or three signatures; the extra ones
/// are co-signers. They authenticate the transaction, none of them identifies
/// it.
///
/// Which matters beyond tidiness: `signature` is part of the unique key of
/// every event table, and a re-ingestion re-fetches by
/// `getTransaction(signature)`. Keying rows on any other element would file
/// them under something no lookup can find — and under something that moves if
/// the co-signer set changes.
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

/// Flatten `meta.innerInstructions` into the ordered payload list an
/// [`OnChainTransaction`] owes its readers.
///
/// Groups are sorted by the outer instruction they belong to, never left in
/// whatever order the RPC serialized them — the order is the `event_index`
/// contract documented on [`OnChainTransaction::inner_instructions`].
///
/// Instructions the RPC returns in another shape than `PartiallyDecoded` are
/// dropped, and so are those whose `data` is not valid base58. Both are
/// representations this adapter cannot turn into bytes, not judgements about
/// what the payload means: `Parsed` proper is produced for instructions the
/// RPC's own parser recognizes (SPL Token and friends), which Anchor self-CPI
/// instructions never are. Narrowing this any further renumbers stored events —
/// see [`InnerInstructionPayload`].
///
/// # The base58 decoding this costs, knowingly
///
/// [`OnChainTransaction`] is program-agnostic, so `data` is decoded for every
/// payload, where the code before it decoded only those already matched to the
/// target program —
/// the program filter now runs downstream, in `extract_anchor_event_cpis`.
/// Measured on the fixture corpus: 74 decodes instead of 60, worst case 8
/// instead of 2 (`zap_protocol_fee.json`, a router-shaped transaction). It is
/// pure waste for the DLMM stub, which discards the transaction entirely.
///
/// Accepted rather than optimised: the alternative is to keep the payload
/// encoded and decode on demand, which puts "this arrived as base58" — a
/// property of one transport — back into the neutral type every other source
/// must fill. That trade is the whole point of the type; a bounded handful of
/// small decodes per transaction is not.
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
#[path = "transaction_adapter_tests.rs"]
mod tests;

// The two suites that drive the whole pipeline from a mainnet fixture. They sit
// beside the adapter because it is what turns a fixture into something
// extraction can read — and they are unit tests rather than `tests/` targets
// because this crate is a binary: an integration target could not reach a
// `pub(crate)` adapter without making it public for the tests' sake.
#[cfg(test)]
#[path = "fixture_pipeline_tests.rs"]
mod fixture_pipeline_tests;

#[cfg(test)]
#[path = "extraction_oracle_tests.rs"]
mod extraction_oracle_tests;
