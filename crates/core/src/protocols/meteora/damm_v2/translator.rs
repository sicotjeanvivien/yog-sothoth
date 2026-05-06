//! Translate DAMM v2 wire events into protocol-agnostic domain events.
//!
//! Wire events ([`super::events::DammV2WireEvent`]) are byte-perfect mirrors
//! of cp-amm's on-chain Anchor events. Domain events
//! ([`crate::domain::DomainEvent`]) are protocol-agnostic representations
//! consumed by the indexer service.
//!
//! Some wire events do not carry every piece of information the domain
//! representation needs. Specifically, [`super::events::EvtSwap2`] and
//! [`super::events::EvtLiquidityChange`] do not include the pool's mint
//! addresses — they assume the caller can recover them from elsewhere.
//! This translator extracts them from the transferChecked CPI instructions
//! sitting alongside each Anchor self-CPI in the same inner instruction
//! group.

use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use solana_transaction_status::{
    EncodedConfirmedTransactionWithStatusMeta, UiInstruction, UiParsedInstruction,
    option_serializer::OptionSerializer,
};
use std::str::FromStr;

use crate::{
    domain::{
        ClaimPositionFeeEvent, ClaimRewardEvent, LiquidityEvent, LiquidityEventKind, Protocol,
        SwapEvent, TradeDirection,
    },
    error::TranslationError,
};

use super::events::{
    DammV2WireEvent, EvtClaimPositionFee, EvtClaimReward, EvtLiquidityChange, EvtSwap2,
};

/// Per-event context required to fully translate Swap2 and LiquidityChange.
///
/// The two mints and the signature/timestamp are extracted once at the
/// orchestration level (in `MeteoraDammV2::extract_events`) and threaded
/// through to the per-event translation functions.
pub(super) struct EventContext {
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub signature: String,
    pub timestamp: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Per-variant translators (option C)
// ---------------------------------------------------------------------------

/// Translate an [`EvtSwap2`] into a [`SwapEvent`].
///
/// Returns `Err` only if `trade_direction` is invalid (out of range).
pub(super) fn translate_swap(
    wire: &EvtSwap2,
    ctx: &EventContext,
) -> Result<SwapEvent, TranslationError> {
    let trade_direction = TradeDirection::from_u8(wire.trade_direction).map_err(|raw| {
        TranslationError::InvalidEnum {
            field: "trade_direction",
            value: raw,
        }
    })?;

    let fee_token_is_a =
        compute_fee_token_is_a(wire.collect_fee_mode, trade_direction).map_err(|raw| {
            TranslationError::InvalidEnum {
                field: "collect_fee_mode",
                value: raw,
            }
        })?;

    // EvtSwap2 reports input/output amounts in
    // `included_transfer_fee_amount_in` / `included_transfer_fee_amount_out`.
    // Map them onto (amount_a, amount_b) according to trade direction:
    //   AtoB → input is on a, output is on b
    //   BtoA → input is on b, output is on a
    let (amount_a, amount_b) = match trade_direction {
        TradeDirection::AtoB => (
            wire.included_transfer_fee_amount_in,
            wire.included_transfer_fee_amount_out,
        ),
        TradeDirection::BtoA => (
            wire.included_transfer_fee_amount_out,
            wire.included_transfer_fee_amount_in,
        ),
    };

    Ok(SwapEvent {
        pool_address: wire.pool,
        protocol: Protocol::MeteoraDammV2,
        signature: ctx.signature.clone(),
        timestamp: ctx.timestamp,

        token_a_mint: ctx.token_a_mint,
        token_b_mint: ctx.token_b_mint,

        trade_direction,
        amount_a,
        amount_b,

        reserve_a_after: wire.reserve_a_amount,
        reserve_b_after: wire.reserve_b_amount,
        next_sqrt_price: wire.swap_result.next_sqrt_price,

        claiming_fee: wire.swap_result.claiming_fee,
        protocol_fee: wire.swap_result.protocol_fee,
        compounding_fee: wire.swap_result.compounding_fee,
        referral_fee: wire.swap_result.referral_fee,
        fee_token_is_a,
    })
}

/// Translate an [`EvtLiquidityChange`] into a [`LiquidityEvent`].
pub(super) fn translate_liquidity(
    wire: &EvtLiquidityChange,
    ctx: &EventContext,
) -> Result<LiquidityEvent, TranslationError> {
    let liquidity_event_kind = LiquidityEventKind::from_u8(wire.change_type).map_err(|raw| {
        TranslationError::InvalidEnum {
            field: "change_type",
            value: raw,
        }
    })?;

    Ok(LiquidityEvent {
        pool_address: wire.pool,
        protocol: Protocol::MeteoraDammV2,
        signature: ctx.signature.clone(),
        timestamp: ctx.timestamp,

        token_a_mint: ctx.token_a_mint,
        token_b_mint: ctx.token_b_mint,

        liquidity_event_kind,
        amount_a: wire.token_a_amount,
        amount_b: wire.token_b_amount,
        liquidity_delta: wire.liquidity_delta,

        reserve_a_after: wire.reserve_a_amount,
        reserve_b_after: wire.reserve_b_amount,

        position: wire.position,
        owner: wire.owner,
    })
}

/// Translate an [`EvtClaimPositionFee`] into a [`ClaimPositionFeeEvent`].
///
/// This translation is infallible — every field maps directly.
pub(super) fn translate_claim_position_fee(
    wire: &EvtClaimPositionFee,
    signature: String,
    timestamp: DateTime<Utc>,
) -> ClaimPositionFeeEvent {
    ClaimPositionFeeEvent {
        pool_address: wire.pool,
        protocol: Protocol::MeteoraDammV2,
        signature,
        timestamp,

        position: wire.position,
        owner: wire.owner,

        fee_a_claimed: wire.fee_a_claimed,
        fee_b_claimed: wire.fee_b_claimed,
    }
}

/// Translate an [`EvtClaimReward`] into a [`ClaimRewardEvent`].
///
/// This translation is infallible — every field maps directly.
pub(super) fn translate_claim_reward(
    wire: &EvtClaimReward,
    signature: String,
    timestamp: DateTime<Utc>,
) -> ClaimRewardEvent {
    ClaimRewardEvent {
        pool_address: wire.pool,
        protocol: Protocol::MeteoraDammV2,
        signature,
        timestamp,

        position: wire.position,
        owner: wire.owner,

        mint_reward: wire.mint_reward,
        reward_index: wire.reward_index,
        total_reward: wire.total_reward,
    }
}

// ---------------------------------------------------------------------------
// Mint extraction from transferChecked context
// ---------------------------------------------------------------------------

/// Extract the canonical (token_a, token_b) mints from the transferChecked
/// instructions inside an inner-instruction group.
///
/// The two mints are sorted by raw pubkey bytes — same convention as
/// [`crate::domain::Pool`]. Stable across swap directions.
///
/// Returns an error if fewer than 2 transferChecked instructions are found
/// in the group.
pub(super) fn extract_mint_pair(
    group_instructions: &[UiInstruction],
) -> Result<(Pubkey, Pubkey), TranslationError> {
    let mints: Vec<Pubkey> = group_instructions
        .iter()
        .filter_map(extract_mint_from_transfer_checked)
        .take(2)
        .collect();

    if mints.len() < 2 {
        return Err(TranslationError::MissingTransferContext(format!(
            "expected at least 2 transferChecked, found {}",
            mints.len()
        )));
    }

    let (m1, m2) = (mints[0], mints[1]);
    if m1 <= m2 { Ok((m1, m2)) } else { Ok((m2, m1)) }
}

/// Try to extract the mint pubkey from a parsed transferChecked instruction.
/// Returns `None` if the instruction is not a transferChecked or is malformed.
fn extract_mint_from_transfer_checked(ix: &UiInstruction) -> Option<Pubkey> {
    let UiInstruction::Parsed(UiParsedInstruction::Parsed(p)) = ix else {
        return None;
    };

    if p.parsed.get("type").and_then(|t| t.as_str()) != Some("transferChecked") {
        return None;
    }

    let mint_str = p
        .parsed
        .get("info")
        .and_then(|info| info.get("mint"))
        .and_then(|m| m.as_str())?;

    Pubkey::from_str(mint_str).ok()
}

/// Walk the inner instruction groups and locate, for each Anchor self-CPI
/// that targets the cp-amm program, the slice of instructions in the same
/// group **before** that self-CPI. Those instructions contain the
/// transferChecked CPIs we need for mint extraction.
///
/// Returns one `Vec<&UiInstruction>` per Anchor self-CPI, in the order the
/// self-CPIs appear across the whole transaction. The length of the returned
/// vector matches the number of `DammV2WireEvent`s produced by the
/// extractor for the same transaction — so callers can zip them by index.
pub(super) fn collect_pre_event_instruction_slices<'a>(
    tx: &'a EncodedConfirmedTransactionWithStatusMeta,
    target_program_id: &str,
) -> Vec<Vec<&'a UiInstruction>> {
    let Some(meta) = tx.transaction.meta.as_ref() else {
        return Vec::new();
    };

    let OptionSerializer::Some(inner_groups) = &meta.inner_instructions else {
        return Vec::new();
    };

    let mut out: Vec<Vec<&UiInstruction>> = Vec::new();

    for group in inner_groups {
        let mut current_slice: Vec<&UiInstruction> = Vec::new();

        for ix in &group.instructions {
            if is_self_cpi_to_program(ix, target_program_id) {
                // Self-CPI marker — emit the slice accumulated so far.
                out.push(std::mem::take(&mut current_slice));
            } else {
                current_slice.push(ix);
            }
        }
    }

    out
}

/// 8-byte tag prefixing every Anchor event_cpi self-CPI's instruction data.
/// Equal to `sha256("anchor:event")[..8]`.
///
/// This is the discriminator Anchor uses for the synthetic instruction it
/// emits when calling `emit_cpi!`. It distinguishes event self-CPIs from
/// regular instructions invoking the same program — both share the same
/// `programId` but differ in their `data` prefix.
const EVENT_IX_TAG: [u8; 8] = [0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d];

/// Returns `true` if the instruction is an Anchor event_cpi self-CPI to the
/// target program.
///
/// The check has two prongs:
///   1. The `programId` must match the target.
///   2. The `data` payload (base58-encoded) must start with [`EVENT_IX_TAG`].
///
/// Both prongs are necessary: the outer `Swap2` instruction targeting the
/// same program also has `programId == target`, but its data prefix is
/// the swap2 instruction discriminator, not the Anchor event tag. Without
/// the second check, mid-transaction routers (Jupiter-style aggregators)
/// would yield false positives at every swap-to-cp-amm hop.
fn is_self_cpi_to_program(ix: &UiInstruction, target_program_id: &str) -> bool {
    let UiInstruction::Parsed(UiParsedInstruction::PartiallyDecoded(p)) = ix else {
        return false;
    };

    if p.program_id != target_program_id {
        return false;
    }

    let data_bytes = match bs58::decode(&p.data).into_vec() {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };

    data_bytes.len() >= 8 && data_bytes[..8] == EVENT_IX_TAG
}

// ---------------------------------------------------------------------------
// Compute fee_token_is_a
// ---------------------------------------------------------------------------

/// Determine whether the fee was charged on token A (`true`) or token B
/// (`false`), based on the on-chain `collect_fee_mode` and the swap's
/// `trade_direction`.
///
/// Mirrors `cp-amm::FeeMode::get_fee_mode` — see source comments in
/// `cp-amm/programs/cp-amm/src/state/fee.rs`. Updated alongside cp-amm
/// upgrades.
fn compute_fee_token_is_a(
    collect_fee_mode: u8,
    trade_direction: TradeDirection,
) -> Result<bool, u8> {
    // CollectFeeMode mapping (mirrors cp-amm enum):
    //   0 = BothToken    — fee on the OUT token
    //   1 = OnlyB        — fee always on token B
    //   2 = Compounding  — fee always on token B
    let fee_token_is_a = match (collect_fee_mode, trade_direction) {
        (0, TradeDirection::AtoB) => false, // out is B, fee on B
        (0, TradeDirection::BtoA) => true,  // out is A, fee on A
        (1, _) => false,                    // OnlyB → always B
        (2, _) => false,                    // Compounding → always B
        (other, _) => return Err(other),
    };
    Ok(fee_token_is_a)
}

// ---------------------------------------------------------------------------
// High-level dispatch
// ---------------------------------------------------------------------------

/// Translate a single wire event into a domain event, given its context.
///
/// `transferChecked_group` is the slice of instructions immediately
/// preceding the self-CPI of this wire event in its inner instruction
/// group. It is used to extract the (token_a, token_b) mint pair for
/// Swap2 and LiquidityChange events. ClaimPositionFee and ClaimReward
/// don't need it.
pub(super) fn translate_wire_event(
    wire: &DammV2WireEvent,
    transfer_checked_group: &[&UiInstruction],
    signature: &str,
    timestamp: DateTime<Utc>,
) -> Result<crate::domain::DomainEvent, TranslationError> {
    use crate::domain::DomainEvent;

    match wire {
        DammV2WireEvent::Swap2(e) => {
            let (token_a_mint, token_b_mint) = extract_mint_pair_from_refs(transfer_checked_group)?;
            let ctx = EventContext {
                token_a_mint,
                token_b_mint,
                signature: signature.to_string(),
                timestamp,
            };
            translate_swap(e, &ctx).map(DomainEvent::Swap)
        }
        DammV2WireEvent::LiquidityChange(e) => {
            let (token_a_mint, token_b_mint) = extract_mint_pair_from_refs(transfer_checked_group)?;
            let ctx = EventContext {
                token_a_mint,
                token_b_mint,
                signature: signature.to_string(),
                timestamp,
            };
            translate_liquidity(e, &ctx).map(DomainEvent::Liquidity)
        }
        DammV2WireEvent::ClaimPositionFee(e) => Ok(DomainEvent::ClaimPositionFee(
            translate_claim_position_fee(e, signature.to_string(), timestamp),
        )),
        DammV2WireEvent::ClaimReward(e) => Ok(DomainEvent::ClaimReward(translate_claim_reward(
            e,
            signature.to_string(),
            timestamp,
        ))),
    }
}

/// Adapter: extract the mint pair from a slice of `&UiInstruction`s
/// (rather than owned `UiInstruction`s as in `extract_mint_pair`).
fn extract_mint_pair_from_refs(
    refs: &[&UiInstruction],
) -> Result<(Pubkey, Pubkey), TranslationError> {
    let owned: Vec<UiInstruction> = refs.iter().map(|r| (*r).clone()).collect();
    extract_mint_pair(&owned)
}
