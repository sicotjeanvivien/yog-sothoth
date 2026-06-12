use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// Pool genesis — emitted once when a DAMM v2 pool is created.
///
/// The authoritative source of a pool's birth parameters: both mints, the
/// initial AMM state (sqrt price and its valid bounds, seeded liquidity), the
/// activation schedule, and the seeded token amounts. Unlike the swap/liquidity
/// flow events, the mints are carried directly, so this can register the pool
/// authoritatively in the registry.
///
/// `pool_fees_raw` holds the borsh-serialized `PoolFeeParameters` blob,
/// captured **undecoded** ("voie C"): the fee schedule (and the `fee_tier`
/// derived from it) is interpreted later by dedicated work, reading from these
/// stored bytes rather than re-indexing.
///
/// `sqrt_*`, `liquidity` are lossless `u128` (`NUMERIC(39, 0)` at the
/// persistence boundary).
///
/// Unlike the swap/liquidity events, the mints here are **not** re-sorted to
/// the canonical raw-byte order: cp-amm does not sort them, and `sqrt_price`
/// and its bounds are tied to the program's native token_a/token_b
/// orientation (re-sorting would require inverting the price). This event
/// therefore preserves the on-chain A/B designation; the cross-protocol
/// `pools` registry is the surface that normalizes to canonical order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2InitializePoolEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,

    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub creator: Pubkey,
    pub payer: Pubkey,
    pub alpha_vault: Pubkey,

    pub sqrt_min_price: u128,
    pub sqrt_max_price: u128,
    pub sqrt_price: u128,
    pub liquidity: u128,

    pub activation_type: u8,
    pub activation_point: u64,
    pub collect_fee_mode: u8,
    pub pool_type: u8,

    pub token_a_flag: u8,
    pub token_b_flag: u8,
    pub token_a_amount: u64,
    pub token_b_amount: u64,
    pub total_amount_a: u64,
    pub total_amount_b: u64,

    /// Raw borsh bytes of the on-chain `PoolFeeParameters` — undecoded.
    pub pool_fees_raw: Vec<u8>,
}
