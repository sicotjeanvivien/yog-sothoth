//! The rotation run: a pool's tranches whose quoted reserve changes hands at least once, no more
//! than [`RUN_GAP`] apart, from the start of the first to the end of the last.

use chrono::TimeDelta;
use rust_decimal::Decimal;
use solana_pubkey::{Pubkey, pubkey};

/// A tranche is marked when its quoted side traded this many times its mean reserve.
pub const RUN_ROTATION: Decimal = Decimal::ONE;

/// The longest gap between two marked tranches of one run; past it, the run is extinct.
pub const RUN_GAP: TimeDelta = TimeDelta::minutes(30);

/// The quoted tokens: SOL, USDC and USDT.
pub const QUOTED_MINTS: [Pubkey; 3] = [
    pubkey!("So11111111111111111111111111111111111111112"),
    pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"),
    pubkey!("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"),
];
