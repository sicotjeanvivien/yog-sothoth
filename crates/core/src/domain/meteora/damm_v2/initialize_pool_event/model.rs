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
/// The mints are in the program's native token_a/token_b designation, like
/// every other event of this protocol. That orientation is load-bearing:
/// `sqrt_price` and its bounds are expressed against it, so re-ordering the
/// pair would require inverting the price.
///
/// **Nothing re-orders it, here or downstream.** An earlier version of this
/// note said the swap/liquidity events were re-sorted to a raw-byte order and
/// that the cross-protocol `pools` registry "normalizes to canonical order";
/// neither was ever true, and the second is the more misleading, since it
/// invites a reader to rely on a normalisation step that does not exist. The
/// registry is filled by yog-context reading the on-chain account, in the
/// program's order — see [`crate::domain::MeteoraDammV2SwapEvent`] for what
/// that order guarantees and what it does not.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2InitializePoolEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    /// Position in the chain — see [`crate::domain::EventPosition`].
    pub slot: u64,
    pub transaction_index: Option<u32>,
    pub event_index: u16,

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
