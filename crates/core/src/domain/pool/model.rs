use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;

use crate::domain::Protocol;

/// A discovered pool — identity and stable metadata.
///
/// Yog-Sothoth observes entire protocols, so pools are discovered on the fly
/// as they appear in the transaction stream. This struct stores what we know
/// about a pool independently of its state at any given momen
///
/// Rows are upserted on every parsed event: `first_seen_at` is set once on
/// first observation, `last_seen_at` is refreshed on every subsequent event.
///
/// # Mints
///
/// The token mints are a property of the pool, resolved authoritatively from
/// the on-chain pool account by yog-context. They are `None` between a pool's
/// discovery (its address appears in the stream) and that resolution — the
/// indexer no longer infers them from the transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pool {
    /// On-chain address of the AMM pool.
    pub pool_address: Pubkey,

    /// Protocol this pool belongs to (DAMM v2, DAMM v1, DLMM).
    pub protocol: Protocol,

    /// Mint of token A. `None` until resolved by yog-context.
    pub token_a_mint: Option<Pubkey>,

    /// Mint of token B. `None` until resolved by yog-context.
    pub token_b_mint: Option<Pubkey>,

    /// Base trading fee in basis points, decoded from the pool's genesis fee
    /// config (`InitializePool`). `None` until that event is seen (or if the
    /// fee blob failed to decode). For a fee-scheduler pool this is the genesis
    /// cliff, not the live decayed rate.
    pub fee_bps: Option<Decimal>,

    /// Meteora's cut of the trading fee, as a whole percent (0..=100), decoded
    /// from the on-chain `Pool` account. `None` until yog-context resolves it.
    pub protocol_fee_percent: Option<u8>,

    /// A partner's cut of the trading fee, as a whole percent (0..=100), decoded
    /// from the on-chain `Pool` account (often 0). `None` until resolved.
    pub partner_fee_percent: Option<u8>,

    /// A referrer's cut of the trading fee, as a whole percent (0..=100),
    /// decoded from the on-chain `Pool` account (only charged when a swap
    /// carries a referral account). `None` until resolved.
    pub referral_fee_percent: Option<u8>,

    /// When Yog-Sothoth first observed this pool in the transaction stream.
    pub first_seen_at: DateTime<Utc>,

    /// Last time any event touched this pool.
    pub last_seen_at: DateTime<Utc>,
}
