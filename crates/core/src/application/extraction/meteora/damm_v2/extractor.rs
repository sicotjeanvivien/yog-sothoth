//! Extract DAMM v2 wire events from a confirmed transaction's self-CPI payloads.
//! Never logs, never aborts the transaction: each payload ends up decoded,
//! unknown or failed, for the caller to log and count.

use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

use crate::{
    application::extraction::{
        DISCRIMINATOR_LEN, OnChainTransaction, decode_anchor_event_cpi, extract_anchor_event_cpis,
    },
    error::AnchorDecodeError,
};

use super::events::{
    DammV2WireEvent, EvtClaimPositionFee, EvtClaimProtocolFee, EvtClaimReward, EvtClosePosition,
    EvtCreatePosition, EvtFundReward, EvtInitializePool, EvtInitializeReward, EvtLiquidityChange,
    EvtLockPosition, EvtPermanentLockPosition, EvtSetPoolStatus, EvtSplitPosition3, EvtSwap2,
    EvtUpdatePoolFees, EvtUpdateRewardDuration, EvtUpdateRewardFunder,
    EvtWithdrawDeadLiquidityReward, EvtWithdrawIneligibleReward, discriminator_claim_position_fee,
    discriminator_claim_protocol_fee, discriminator_claim_reward, discriminator_close_position,
    discriminator_create_position, discriminator_fund_reward, discriminator_initialize_pool,
    discriminator_initialize_reward, discriminator_liquidity_change, discriminator_lock_position,
    discriminator_permanent_lock_position, discriminator_set_pool_status,
    discriminator_split_position2, discriminator_split_position3, discriminator_swap2,
    discriminator_update_pool_fees, discriminator_update_reward_duration,
    discriminator_update_reward_funder, discriminator_withdraw_dead_liquidity_reward,
    discriminator_withdraw_ineligible_reward,
};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Outcome of extracting wire events from a transaction; never fails as a whole.
#[derive(Debug, Default)]
pub struct ExtractedEvents {
    /// Recognized events, in transaction order.
    pub events: Vec<IndexedWireEvent>,

    /// Payloads whose discriminator matches no known event: a metric, not an error.
    pub unknown: Vec<UnknownEvent>,

    /// Payloads addressed to cp-amm that failed to decode.
    pub failures: Vec<ExtractFailure>,
}

/// A recognized wire event and its position in the transaction.
/// ⚠️ `event_index` counts **every** self-CPI payload, skipped ones included: that
/// keeps it stable across releases ([`crate::domain::EventPosition`]).
#[derive(Debug)]
pub struct IndexedWireEvent {
    pub event_index: u16,
    pub event: DammV2WireEvent,
}

/// A self-CPI payload whose discriminator matches no known event; the
/// discriminator labels the caller's metric.
#[derive(Debug, Clone, Copy)]
pub struct UnknownEvent {
    pub discriminator: [u8; DISCRIMINATOR_LEN],
}

/// A failed extraction attempt and its reason.
#[derive(Debug, thiserror::Error)]
pub enum ExtractFailure {
    /// Neither an ordinary instruction nor a decodable event_cpi payload.
    #[error("anchor event_cpi decode failed: {source}")]
    AnchorDecode {
        #[source]
        source: AnchorDecodeError,
    },

    /// Known discriminator, but borsh failed: usually schema drift in our mirror.
    #[error("borsh deserialization of {event_name} failed: {reason}")]
    Borsh {
        event_name: &'static str,
        discriminator: [u8; DISCRIMINATOR_LEN],
        reason: String,
    },

    /// More self-CPI payloads than an `event_index` can number.
    /// ⚠️ Dropped, never truncated: a wrong index is a duplicate or a lost row.
    #[error("event index {index} exceeds u16")]
    EventIndexOverflow { index: usize },
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Extract every DAMM v2 wire event the transaction emitted to `program_id`.
pub fn extract_wire_events(tx: &OnChainTransaction, program_id: &Pubkey) -> ExtractedEvents {
    let raw_payloads = extract_anchor_event_cpis(tx, program_id);

    // Pre-compute discriminators once per transaction.
    let known = KnownDiscriminators {
        swap2: discriminator_swap2(),
        liquidity: discriminator_liquidity_change(),
        claim_pos_fee: discriminator_claim_position_fee(),
        claim_reward: discriminator_claim_reward(),
        claim_protocol_fee: discriminator_claim_protocol_fee(),
        initialize_reward: discriminator_initialize_reward(),
        fund_reward: discriminator_fund_reward(),
        withdraw_ineligible_reward: discriminator_withdraw_ineligible_reward(),
        update_reward_duration: discriminator_update_reward_duration(),
        update_reward_funder: discriminator_update_reward_funder(),
        withdraw_dead_liquidity_reward: discriminator_withdraw_dead_liquidity_reward(),
        split_position3: discriminator_split_position3(),
        split_position2_deprecated: discriminator_split_position2(),
        create_position: discriminator_create_position(),
        close_position: discriminator_close_position(),
        lock_position: discriminator_lock_position(),
        permanent_lock_position: discriminator_permanent_lock_position(),
        initialize_pool: discriminator_initialize_pool(),
        set_pool_status: discriminator_set_pool_status(),
        update_pool_fees: discriminator_update_pool_fees(),
    };
    let mut out = ExtractedEvents::default();

    for (index, raw) in raw_payloads.iter().enumerate() {
        // ⚠️ Numbered before decoding: an unrecognized payload still consumes its index.
        let Ok(event_index) = u16::try_from(index) else {
            out.failures
                .push(ExtractFailure::EventIndexOverflow { index });
            continue;
        };

        match decode_anchor_event_cpi(raw) {
            Ok((disc, body)) => match dispatch(&disc, &body, &known) {
                Dispatch::Recognized(event) => {
                    out.events.push(IndexedWireEvent { event_index, event })
                }
                // Deliberately dropped: recorded nowhere, by design.
                Dispatch::Ignored => {}
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
            Err(AnchorDecodeError::NotAnAnchorEvent) => {}
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
    claim_protocol_fee: [u8; DISCRIMINATOR_LEN],
    initialize_reward: [u8; DISCRIMINATOR_LEN],
    fund_reward: [u8; DISCRIMINATOR_LEN],
    withdraw_ineligible_reward: [u8; DISCRIMINATOR_LEN],
    update_reward_duration: [u8; DISCRIMINATOR_LEN],
    update_reward_funder: [u8; DISCRIMINATOR_LEN],
    withdraw_dead_liquidity_reward: [u8; DISCRIMINATOR_LEN],
    split_position3: [u8; DISCRIMINATOR_LEN],
    /// Recognised only so it can be dropped on purpose — never decoded.
    split_position2_deprecated: [u8; DISCRIMINATOR_LEN],
    create_position: [u8; DISCRIMINATOR_LEN],
    close_position: [u8; DISCRIMINATOR_LEN],
    lock_position: [u8; DISCRIMINATOR_LEN],
    permanent_lock_position: [u8; DISCRIMINATOR_LEN],
    initialize_pool: [u8; DISCRIMINATOR_LEN],
    set_pool_status: [u8; DISCRIMINATOR_LEN],
    update_pool_fees: [u8; DISCRIMINATOR_LEN],
}

/// Outcome of dispatching a single decoded event by its discriminator.
enum Dispatch {
    Recognized(DammV2WireEvent),
    /// Known, and deliberately not indexed; [`Dispatch::Unknown`] keeps meaning
    /// "an event type we are missing".
    Ignored,
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
    } else if disc == &known.claim_protocol_fee {
        deserialize::<EvtClaimProtocolFee>(body, "EvtClaimProtocolFee")
            .map(|e| Dispatch::Recognized(DammV2WireEvent::ClaimProtocolFee(e)))
            .unwrap_or_else(|reason| Dispatch::BorshFailed {
                event_name: "EvtClaimProtocolFee",
                reason,
            })
    } else if disc == &known.initialize_reward {
        deserialize::<EvtInitializeReward>(body, "EvtInitializeReward")
            .map(|e| Dispatch::Recognized(DammV2WireEvent::InitializeReward(e)))
            .unwrap_or_else(|reason| Dispatch::BorshFailed {
                event_name: "EvtInitializeReward",
                reason,
            })
    } else if disc == &known.fund_reward {
        deserialize::<EvtFundReward>(body, "EvtFundReward")
            .map(|e| Dispatch::Recognized(DammV2WireEvent::FundReward(e)))
            .unwrap_or_else(|reason| Dispatch::BorshFailed {
                event_name: "EvtFundReward",
                reason,
            })
    } else if disc == &known.withdraw_ineligible_reward {
        deserialize::<EvtWithdrawIneligibleReward>(body, "EvtWithdrawIneligibleReward")
            .map(|e| Dispatch::Recognized(DammV2WireEvent::WithdrawIneligibleReward(e)))
            .unwrap_or_else(|reason| Dispatch::BorshFailed {
                event_name: "EvtWithdrawIneligibleReward",
                reason,
            })
    } else if disc == &known.update_reward_duration {
        deserialize::<EvtUpdateRewardDuration>(body, "EvtUpdateRewardDuration")
            .map(|e| Dispatch::Recognized(DammV2WireEvent::UpdateRewardDuration(e)))
            .unwrap_or_else(|reason| Dispatch::BorshFailed {
                event_name: "EvtUpdateRewardDuration",
                reason,
            })
    } else if disc == &known.update_reward_funder {
        deserialize::<EvtUpdateRewardFunder>(body, "EvtUpdateRewardFunder")
            .map(|e| Dispatch::Recognized(DammV2WireEvent::UpdateRewardFunder(e)))
            .unwrap_or_else(|reason| Dispatch::BorshFailed {
                event_name: "EvtUpdateRewardFunder",
                reason,
            })
    } else if disc == &known.withdraw_dead_liquidity_reward {
        deserialize::<EvtWithdrawDeadLiquidityReward>(body, "EvtWithdrawDeadLiquidityReward")
            .map(|e| Dispatch::Recognized(DammV2WireEvent::WithdrawDeadLiquidityReward(e)))
            .unwrap_or_else(|reason| Dispatch::BorshFailed {
                event_name: "EvtWithdrawDeadLiquidityReward",
                reason,
            })
    } else if disc == &known.split_position3 {
        deserialize::<EvtSplitPosition3>(body, "EvtSplitPosition3")
            .map(|e| Dispatch::Recognized(DammV2WireEvent::SplitPosition3(Box::new(e))))
            .unwrap_or_else(|reason| Dispatch::BorshFailed {
                event_name: "EvtSplitPosition3",
                reason,
            })
    } else if disc == &known.split_position2_deprecated {
        // ⚠️ cp-amm emits v2 beside v3 on every split; indexing both double-counts it.
        Dispatch::Ignored
    } else if disc == &known.create_position {
        deserialize::<EvtCreatePosition>(body, "EvtCreatePosition")
            .map(|e| Dispatch::Recognized(DammV2WireEvent::CreatePosition(e)))
            .unwrap_or_else(|reason| Dispatch::BorshFailed {
                event_name: "EvtCreatePosition",
                reason,
            })
    } else if disc == &known.close_position {
        deserialize::<EvtClosePosition>(body, "EvtClosePosition")
            .map(|e| Dispatch::Recognized(DammV2WireEvent::ClosePosition(e)))
            .unwrap_or_else(|reason| Dispatch::BorshFailed {
                event_name: "EvtClosePosition",
                reason,
            })
    } else if disc == &known.lock_position {
        deserialize::<EvtLockPosition>(body, "EvtLockPosition")
            .map(|e| Dispatch::Recognized(DammV2WireEvent::LockPosition(e)))
            .unwrap_or_else(|reason| Dispatch::BorshFailed {
                event_name: "EvtLockPosition",
                reason,
            })
    } else if disc == &known.permanent_lock_position {
        deserialize::<EvtPermanentLockPosition>(body, "EvtPermanentLockPosition")
            .map(|e| Dispatch::Recognized(DammV2WireEvent::PermanentLockPosition(e)))
            .unwrap_or_else(|reason| Dispatch::BorshFailed {
                event_name: "EvtPermanentLockPosition",
                reason,
            })
    } else if disc == &known.initialize_pool {
        deserialize::<EvtInitializePool>(body, "EvtInitializePool")
            .map(|e| Dispatch::Recognized(DammV2WireEvent::InitializePool(Box::new(e))))
            .unwrap_or_else(|reason| Dispatch::BorshFailed {
                event_name: "EvtInitializePool",
                reason,
            })
    } else if disc == &known.set_pool_status {
        deserialize::<EvtSetPoolStatus>(body, "EvtSetPoolStatus")
            .map(|e| Dispatch::Recognized(DammV2WireEvent::SetPoolStatus(e)))
            .unwrap_or_else(|reason| Dispatch::BorshFailed {
                event_name: "EvtSetPoolStatus",
                reason,
            })
    } else if disc == &known.update_pool_fees {
        deserialize::<EvtUpdatePoolFees>(body, "EvtUpdatePoolFees")
            .map(|e| Dispatch::Recognized(DammV2WireEvent::UpdatePoolFees(e)))
            .unwrap_or_else(|reason| Dispatch::BorshFailed {
                event_name: "EvtUpdatePoolFees",
                reason,
            })
    } else {
        Dispatch::Unknown
    }
}

/// Borsh-deserialize a wire event; the error message goes into a typed `ExtractFailure`.
fn deserialize<T: BorshDeserialize>(body: &[u8], _event_name: &'static str) -> Result<T, String> {
    T::try_from_slice(body).map_err(|e| e.to_string())
}
