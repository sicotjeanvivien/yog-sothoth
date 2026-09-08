//! Yellowstone protobuf adapter: `SubscribeUpdateTransaction` →
//! [`OnChainTransaction`].
//!
//! The second of the two adapters that fill the neutral transaction `yog-core`
//! extracts from. It sits beside `infra::rpc::transaction_adapter` for the same
//! reason that one sits where it does: `core` has no business naming a
//! transport, so each source's schema stops here.
//!
//! What it owes the rest of the workspace is stated once, in
//! `yog_core::application::extraction::conformance`, which both adapters are
//! arbitrated by.
//!
//! # ⚠️ What green here does **not** prove
//!
//! This module's tests build their input by hand. There is no mainnet protobuf
//! fixture in this repository and no way to make one without a subscription, so
//! **the message is constructed with the same understanding the code uses to
//! read it, and the two can agree on a lie.** That is not a hypothetical: the
//! pool-properties decoder shipped fourteen green synthetic tests over a wrong
//! offset, and only a real mainnet account exposed it.
//!
//! Three things are therefore asserted here and established nowhere:
//!
//! 1. that a provider's `SubscribeUpdate` looks like the ones below — how inner
//!    instructions are grouped, what `data` carries exactly;
//! 2. that [`resolve_program_id`] walks the account list the way a validator
//!    does. **No fixture in this repository can witness it**, and not by
//!    accident: 25 of the 92 transaction fixtures *do* use address lookup
//!    tables, yet **none** carries a `loadedAddresses` in its captured response
//!    — and it would change nothing if they did, since a JSON-RPC response
//!    hands `programId` over already resolved as a string. Index resolution is
//!    structurally a gRPC-only concern, so the corpus could never exercise it.
//!    The test that covers it is built from the rule as documented, not from an
//!    observation;
//! 3. anything temporal — this module sees one message and has no clock.
//!
//! # ⚠️ Two things this adapter deliberately does not do, for the listener slice
//!
//! **It does not look at `meta.err`**, so events from reverted transactions
//! would be persisted if nothing upstream filtered them. On the JSON-RPC path that
//! filtering lives in `infra/rpc/dispatcher/filters/failed_transaction.rs`, and
//! the corpus keeps `damm_v2/swap_failed.json` for it. The gRPC path has no
//! counterpart yet: the listener must set `failed: Some(false)` on its
//! `SubscribeRequestFilterTransactions` — pushing the filter server-side, which
//! is one of the stated gains — or check `meta.err` here.
//!
//! **It does not look at `is_vote` either**, and the omission bites twice. A
//! subscription without `vote: Some(false)` hands over the bulk of the stream:
//! every vote update gets its signature decoded, its index narrowed and its
//! payloads allocated, to be discarded downstream. And under the rule two
//! paragraphs up, a provider that reports votes with `inner_instructions_none`
//! set turns each one into a **counted skip-and-log failure** — a metric that
//! would read as a broken pipeline while nothing is wrong.
//!
//! Both are written down because a filter nobody remembers is a filter nobody
//! adds. Raised in review, 8 September 2026.
//!
//! Their first confrontation with reality is
//! `02 - backlog/pre-v02/flux-grpc-reel-mesures.md`, which needs an API key.
//! Until then, read this suite as "the translation is self-consistent", never
//! as "the translation is right".

// ⚠️ The whole module is unreachable until the listener lands — slice 3 of
// `03 - active/listener-grpc-yellowstone.md`. `RpcListener::_watch`'s `_`
// convention covers a single item whose neighbours are live; here every helper
// is dead because the entry point is, so one marker carrying the reason beats
// seven prefixes to strip later. Removing this line is part of wiring the
// listener, and the build says so the moment it is.
#![allow(dead_code)]

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

/// Build the transport-neutral transaction from a Yellowstone update.
///
/// # Why the timestamp is an argument
///
/// Because the message does not carry one. `SubscribeUpdateTransaction` holds
/// `transaction` and `slot`, full stop; `block_time` lives on
/// `SubscribeUpdateBlockMeta`, a different subscription indexed by slot. And
/// `TransactionPosition::timestamp` may not be optional — it is both a member of
/// every event table's unique key and the TimescaleDB partitioning column, so a
/// missing one is not a degraded row, it is an unwritable one.
///
/// Correlating slot → time, and bounding the wait for a block-meta that may
/// never arrive, is a problem of its own with its own failure modes. It is the
/// next slice of this ticket, and this signature is the seam between them: this
/// function translates one message and knows nothing about time.
///
/// # Errors
///
/// Only on a transaction-level malformation, and this list is what slice 3's
/// listener will read to decide what to log, count and retry — so it is kept
/// complete:
///
/// - the `transaction` envelope is absent;
/// - `signature` is not `SIGNATURE_BYTES` long;
/// - `index` does not fit in the domain's `u32`;
/// - `transaction.message` is absent, so there are no static account keys;
/// - `meta` is absent, or `meta.inner_instructions_none` is set — both mean the
///   source did not capture the inner instructions, which is **not** the same
///   as there being none;
/// - an inner instruction's `program_id_index` points outside the account list;
/// - a resolved account key is not 32 bytes, so it is not a public key.
///
/// ⚠️ That last one was missing until review pointed at it, in a list whose own
/// sentence claims to be complete. A claim of completeness is a claim, and this
/// one is load-bearing: slice 3 reads it to decide what to log and count.
///
/// A transaction that genuinely carries no inner instructions is not a failure:
/// it yields an empty payload list, and extraction reports "nothing to
/// record".
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

/// Read the transaction id from its raw bytes.
///
/// `signature` on the update is the id itself — the provider has already picked
/// `signatures[0]` out of the message, so unlike the JSON-RPC adapter there is
/// no choice to make here and no base58 to decode. The length check is what
/// `Signature::try_from` gives us; anything else is a malformed message.
fn extract_signature(info: &SubscribeUpdateTransactionInfo) -> CoreResult<Signature> {
    Signature::try_from(info.signature.as_slice()).map_err(|_| CoreError::ParseError {
        signature: String::new(),
        // `SIGNATURE_BYTES` and not `size_of::<Signature>()`: the two agree
        // today, but one is a wire constant and the other a layout assumption,
        // and only the first is what the message promises.
        reason: format!(
            "signature is {} bytes, expected {SIGNATURE_BYTES}",
            info.signature.len(),
        ),
    })
}

/// Narrow the transaction's position in its slot to the width the domain uses.
///
/// ⚠️ An out-of-range value is an **error**, never a truncating cast. This field
/// is the whole point of moving to gRPC — it is what disambiguates two
/// transactions of the same slot touching the same pool, an ambiguity measured
/// at 17.8 % of current-state updates. A silently wrapped index would not fail:
/// it would order events wrongly, for ever, in a column nobody re-reads.
///
/// A slot cannot hold anywhere near `u32::MAX` transactions, so this is not
/// expected to fire. It is here because "cannot happen" and "is not checked" are
/// different claims, and only the second one is visible in a cast.
fn extract_index(info: &SubscribeUpdateTransactionInfo, signature: &Signature) -> CoreResult<u32> {
    u32::try_from(info.index).map_err(|_| CoreError::ParseError {
        signature: signature.to_string(),
        reason: format!("transaction index {} does not fit in u32", info.index),
    })
}

/// Flatten `meta.inner_instructions` into the ordered payload list an
/// [`OnChainTransaction`] owes its readers.
///
/// Groups are sorted by the outer instruction they belong to, exactly as the
/// JSON-RPC adapter does — the order is the `event_index` contract documented on
/// [`OnChainTransaction::inner_instructions`], and two adapters filling the same
/// vector in two orders is how stored events get renumbered.
///
/// Nothing is filtered. Where the JSON-RPC adapter must drop instructions it
/// cannot represent — a shape its encoding renders differently, a `data` that is
/// not valid base58 — protobuf has neither problem: `data` is already bytes.
/// Every inner instruction becomes a payload, and the program filter runs
/// downstream where it belongs.
fn extract_inner_instructions(
    info: &SubscribeUpdateTransactionInfo,
    signature: &Signature,
) -> CoreResult<Vec<InnerInstructionPayload>> {
    // ⚠️ An absent `meta` is the **same** absence as the flag below, and was
    // treated as its opposite until review caught it: both say "the source did
    // not tell us", and returning an empty list records a transaction full of
    // events as "nothing to record". `meta` is genuinely optional on the wire —
    // a relay could strip it — and unlike the JSON-RPC sibling, no fixture
    // corpus here can show what a provider actually sends. Refusing is what
    // puts it on the skip-and-log path instead of losing it.
    let Some(meta) = info.meta.as_ref() else {
        return Err(CoreError::MissingField {
            signature: signature.to_string(),
            field: "meta (not captured by the source)".to_string(),
        });
    };

    // ⚠️ `inner_instructions_none` is not "there were none" — it is "the source
    // did not capture them", which the proto carries a separate flag for
    // precisely because the two must not be confused. Reading it as an empty
    // list would record a transaction full of events as "nothing to record":
    // no error, no metric, no retry, every event in it lost without a trace.
    // Refusing sends it down the skip-and-log path, where a per-transaction
    // failure is counted and stepped over. Found in review, 8 September 2026.
    if meta.inner_instructions_none {
        return Err(CoreError::MissingField {
            signature: signature.to_string(),
            field: "meta.inner_instructions (not captured by the source)".to_string(),
        });
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

/// The three account-key segments an instruction index is resolved against, in
/// the order a validator resolves them.
///
/// **static keys, then loaded writable, then loaded readonly.** The order is not
/// a guess: it is what `solana_message::AccountKeys::get` walks, whose own
/// documentation says the segment ordering "affects how account indexes from
/// compiled instructions are resolved and so should not be changed".
///
/// Written here rather than by depending on `solana-message`: the lock file
/// already carries two versions of `solana-address` and two of `solana-pubkey`,
/// and threading a third link through that knot costs more than ten lines.
/// ⚠️ A missing message is an **error**, not an empty first segment. Found in
/// review, 8 September 2026: `map_or(&[][..], …)` turned an absent envelope into
/// zero static keys, which shifts every index one segment along — with a
/// non-empty `loaded_writable_addresses`, `program_id_index = 0` then resolves
/// to the first *loaded* key. A valid, wrong `Pubkey`, dropped downstream in
/// silence. That is precisely what [`resolve_program_id`] says it refuses to
/// allow, undone one function earlier.
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
            // Named, not empty: on the skip-and-log path this line is all an
            // operator gets, and an error that cannot say which transaction it
            // is about cannot be investigated.
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
///
/// # ⚠️ Why an out-of-range index is an error and not a skip
///
/// Because this is the one place the translation can lie without saying so. An
/// index resolved against the wrong segment does not fail: it yields a
/// different, perfectly valid public key, which the program filter downstream
/// then discards in silence. The result is zero events extracted, zero errors
/// logged, and a green test suite — the exact shape of failure this repository
/// keeps meeting. So where the value cannot be resolved at all, that is said out
/// loud rather than turned into an absent payload, which would silently
/// renumber every `event_index` after it.
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
#[path = "transaction_adapter_tests.rs"]
mod tests;
