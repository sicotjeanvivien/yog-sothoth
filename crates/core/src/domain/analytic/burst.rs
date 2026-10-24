//! The burst: a pool's tranches whose quoted reserve changes hands at least once, no more than
//! [`BURST_GAP`] apart, from the start of the first to the end of the last.

use chrono::TimeDelta;
use rust_decimal::Decimal;

/// A tranche is marked when its quoted side traded this many times its mean reserve.
pub const BURST_ROTATION: Decimal = Decimal::ONE;

/// The longest gap between two marked tranches of one burst; past it, the burst is extinct.
pub const BURST_GAP: TimeDelta = TimeDelta::minutes(30);

/// The quoted tokens: SOL, USDC and USDT. ⚠️ A pair with both or neither takes the smaller
/// rotation of its two sides.
pub const QUOTED_MINTS: [&str; 3] = [
    "So11111111111111111111111111111111111111112",
    "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
    "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
];
