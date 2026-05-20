//! Extract DAMM v2 wire events from a confirmed Solana transaction.
//!
//! Bridges three pieces:
//! - the generic Anchor event_cpi decoder ([`crate::protocols::anchor_event`])
//! - the DAMM v2 wire event mirrors ([`super::events`])
//! - the Solana transaction format ([`EncodedConfirmedTransactionWithStatusMeta`])
//!
//! The extractor walks every inner instruction targeted at the cp-amm
//! program, decodes those that look like Anchor self-CPI event emissions,
//! and dispatches each one to the matching wire event struct based on its
//! 8-byte discriminator.
//!
//! ## Error handling philosophy
//!
//! `core` is a pure, log-free crate. The extractor never logs and never
//! aborts the whole transaction. Instead, it surfaces three distinct kinds
//! of outcome via [`ExtractedEvents`]:
//!
//! - successfully recognized and decoded events → `events`
//! - inner instructions whose discriminator is unknown to us → `unknown`
//! - inner instructions that match a known discriminator but whose payload
//!   fails to deserialize → `failures`
//!
//! Each failure carries enough structured information for the caller (in
//! `indexer/`) to log it and emit metrics with appropriate cardinality.

use crate::solana_types::EncodedConfirmedTransactionWithStatusMeta;
use borsh::BorshDeserialize;

use crate::{
    error::AnchorDecodeError,
    protocols::anchor_event::{
        DISCRIMINATOR_LEN, decode_anchor_event_cpi, extract_anchor_event_cpis,
    },
};

use super::events::{
    DammV2WireEvent, EvtClaimPositionFee, EvtClaimReward, EvtLiquidityChange, EvtSwap2,
    discriminator_claim_position_fee, discriminator_claim_reward, discriminator_liquidity_change,
    discriminator_swap2,
};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Outcome of extracting wire events from a transaction.
///
/// Always well-formed — extraction never fails as a whole. Per-event
/// problems are reported through `unknown` and `failures`.
#[derive(Debug, Default)]
pub struct ExtractedEvents {
    /// All recognized events, in the order they appeared in the transaction.
    pub events: Vec<DammV2WireEvent>,

    /// Inner instructions whose discriminator did not match any known
    /// event. Most often correspond to events from rings we haven't
    /// implemented yet (CreatePosition, ClaimProtocolFee, etc.) — the
    /// caller should report them as a metric, not as an error.
    pub unknown: Vec<UnknownEvent>,

    /// Inner instructions that targeted the cp-amm program but failed to
    /// be decoded into a wire event. Each entry carries enough context
    /// for the caller to log + emit a metric.
    pub failures: Vec<ExtractFailure>,
}

/// A self-CPI inner instruction whose discriminator does not match any
/// event we know about.
///
/// Carrying the discriminator (rather than just a count) lets the caller
/// emit metrics labelled by discriminator hex — useful for spotting which
/// unrecognized event type appears most often.
#[derive(Debug, Clone, Copy)]
pub struct UnknownEvent {
    pub discriminator: [u8; DISCRIMINATOR_LEN],
}

/// A failed extraction attempt, paired with the reason why.
///
/// The variants are intentionally narrow so the caller can route each
/// kind of failure to the right metric / log level.
#[derive(Debug, thiserror::Error)]
pub enum ExtractFailure {
    /// Inner instruction targets cp-amm but the bytes don't form a valid
    /// Anchor event_cpi payload (wrong tag, payload too short, etc.).
    /// Could indicate a non-event self-CPI we don't recognize, or a
    /// truly malformed payload.
    #[error("anchor event_cpi decode failed: {source}")]
    AnchorDecode {
        #[source]
        source: AnchorDecodeError,
    },

    /// Discriminator matches a known event, but borsh deserialization of
    /// the payload failed. Usually indicates schema drift between cp-amm
    /// and our mirror structs.
    #[error("borsh deserialization of {event_name} failed: {reason}")]
    Borsh {
        event_name: &'static str,
        discriminator: [u8; DISCRIMINATOR_LEN],
        reason: String,
    },
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Extract every DAMM v2 wire event the transaction emitted.
///
/// `program_id_str` is the cp-amm program ID as a base58 string — it must
/// match exactly the program ID that appears in the transaction's inner
/// instructions (e.g. `"cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG"` for
/// mainnet).
///
/// Never errors out. A transaction with no inner instructions, no events
/// for cp-amm, or only events we don't recognize all return a well-formed
/// [`ExtractedEvents`].
pub fn extract_wire_events(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    program_id_str: &str,
) -> ExtractedEvents {
    let raw_payloads = extract_anchor_event_cpis(tx, program_id_str);

    // Pre-compute discriminators once per transaction.
    let known = KnownDiscriminators {
        swap2: discriminator_swap2(),
        liquidity: discriminator_liquidity_change(),
        claim_pos_fee: discriminator_claim_position_fee(),
        claim_reward: discriminator_claim_reward(),
    };
    let mut out = ExtractedEvents::default();

    for raw in raw_payloads {
        match decode_anchor_event_cpi(&raw) {
            Ok((disc, body)) => match dispatch(&disc, &body, &known) {
                Dispatch::Recognized(event) => out.events.push(event),
                Dispatch::Unknown => out.unknown.push(UnknownEvent {
                    discriminator: disc,
                }),
                Dispatch::BorshFailed { event_name, reason } => {
                    out.failures.push(ExtractFailure::Borsh {
                        event_name,
                        discriminator: disc,
                        reason,
                    });
                }
            },
            Err(AnchorDecodeError::NotAnAnchorEvent) => {
                // Not an event_cpi emission — could be a legitimate
                // non-event self-CPI from cp-amm. Not a failure.
            }
            Err(e) => {
                out.failures
                    .push(ExtractFailure::AnchorDecode { source: e });
            }
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/// Pre-computed discriminators for the events we currently mirror.
struct KnownDiscriminators {
    swap2: [u8; DISCRIMINATOR_LEN],
    liquidity: [u8; DISCRIMINATOR_LEN],
    claim_pos_fee: [u8; DISCRIMINATOR_LEN],
    claim_reward: [u8; DISCRIMINATOR_LEN],
}

/// Outcome of dispatching a single decoded event by its discriminator.
enum Dispatch {
    Recognized(DammV2WireEvent),
    Unknown,
    BorshFailed {
        event_name: &'static str,
        reason: String,
    },
}

fn dispatch(disc: &[u8; DISCRIMINATOR_LEN], body: &[u8], known: &KnownDiscriminators) -> Dispatch {
    if disc == &known.swap2 {
        deserialize::<EvtSwap2>(body, "EvtSwap2")
            .map(|e| Dispatch::Recognized(DammV2WireEvent::Swap2(e)))
            .unwrap_or_else(|reason| Dispatch::BorshFailed {
                event_name: "EvtSwap2",
                reason,
            })
    } else if disc == &known.liquidity {
        deserialize::<EvtLiquidityChange>(body, "EvtLiquidityChange")
            .map(|e| Dispatch::Recognized(DammV2WireEvent::LiquidityChange(e)))
            .unwrap_or_else(|reason| Dispatch::BorshFailed {
                event_name: "EvtLiquidityChange",
                reason,
            })
    } else if disc == &known.claim_pos_fee {
        deserialize::<EvtClaimPositionFee>(body, "EvtClaimPositionFee")
            .map(|e| Dispatch::Recognized(DammV2WireEvent::ClaimPositionFee(e)))
            .unwrap_or_else(|reason| Dispatch::BorshFailed {
                event_name: "EvtClaimPositionFee",
                reason,
            })
    } else if disc == &known.claim_reward {
        deserialize::<EvtClaimReward>(body, "EvtClaimReward")
            .map(|e| Dispatch::Recognized(DammV2WireEvent::ClaimReward(e)))
            .unwrap_or_else(|reason| Dispatch::BorshFailed {
                event_name: "EvtClaimReward",
                reason,
            })
    } else {
        Dispatch::Unknown
    }
}

/// Borsh-deserialize a wire event. On failure, returns the error message
/// as a `String` so the caller can wrap it in a typed `ExtractFailure`.
fn deserialize<T: BorshDeserialize>(body: &[u8], _event_name: &'static str) -> Result<T, String> {
    T::try_from_slice(body).map_err(|e| e.to_string())
}
