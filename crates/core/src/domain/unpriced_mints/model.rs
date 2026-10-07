//! Which mints the price source answered without a price, and when to ask it
//! again.
//!
//! The price worker asks for every known mint, and most of them never come
//! back with a price: dead projects and memecoins with no route. Measured on
//! 28 September 2026, 2 970 of the 5 522 known mints had never been priced
//! once, each asked about 4 500 times. Asked in back-to-back chunks, they cost
//! the rate limit that live mints need: 41 % of the calls refused (25 September
//! 2026), and a tick of ~100 s of which ~89 % was spent sleeping on 429s.
//!
//! The source's own answer is the signal, and it is enough: deciding whether to
//! ask again does not need to know *why* a mint has no price (dead, not indexed
//! yet, liquidity stranded in an abandoned pool). It needs to know that the
//! source, asked, had none — which this rule re-checks at least every
//! [`UNPRICED_RETRY_MAX`].
//!
//! When to ask is a product judgement about freshness, like [`KeptPrices`],
//! and lives here for the same reason.
//!
//! [`KeptPrices`]: crate::domain::KeptPrices

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use solana_pubkey::Pubkey;

/// The longest a mint the source answered without a price waits before it is
/// asked again.
///
/// **Short, because a longer one buys nothing.** Projected on the 28 September
/// 2026 universe, a cap of 15 minutes leaves ~47 chunks per tick and a cap of
/// 24 hours ~44: the whole gain comes from no longer asking every tick, not
/// from how long the wait grows. The cap is what bounds the delay of a mint
/// that comes back to life — its swaps go unvalued until it is asked again,
/// and an as-of gap is never backfilled (migration 005). On the 27 September
/// 2026 event, 204 mints lost their price for three hours then regained it; 15
/// minutes of delay would have cost at most the 10 swaps measured in the 15
/// minutes after.
///
/// Its value equals [`PRICE_MAX_AGE_LATEST`] and owes it nothing: a mint with
/// no price has no freshness to keep.
///
/// [`PRICE_MAX_AGE_LATEST`]: crate::domain::PRICE_MAX_AGE_LATEST
const UNPRICED_RETRY_MAX: Duration = Duration::minutes(15);

/// The mints the source last answered without a price, and when each is due
/// again.
///
/// Held by the price worker, the only one to see the source's answers. A mint
/// the source has not answered for — never asked, or asked in a request that
/// failed — is **not** in here, and is due.
///
/// # The wait
///
/// After `n` answers in a row without a price, the mint waits
/// `cadence × 2ⁿ`, up to `UNPRICED_RETRY_MAX`: at the default 30 s cadence,
/// 1, 2, 4, 8, then 15 minutes for as long as it stays without a price. The
/// first waits are short on purpose: a token Jupiter has not indexed yet gets
/// its price within minutes (392 of the 399 mints first priced during the
/// 28 September 2026 window got it within 5 minutes of discovery), and those
/// first minutes are when it trades.
///
/// ⚠️ **The guarantee is the cap plus one cycle, not the cap.** The worker
/// checks a mint at each tick, so a mint due at `t` is asked at the first tick
/// that starts after `t`. A cycle is the cadence, or the tick's own duration
/// when it overruns — ~100 s against 5 000 mints in September 2026.
///
/// # What resets it
///
/// **Any price.** The first answer with a price removes the mint, which goes
/// back to being asked every tick, and a later answer without one starts the
/// waits over from the first. A price the column cannot store counts as a
/// price here: the source had one, and the storability filter is another
/// question.
///
/// **What does not.** A request that failed — a chunk given up on 429 — says
/// nothing about its mints. Counting it as "no price" would slow down mints
/// that have one: ~500 per tick were in that case on 28 September 2026. The
/// caller passes such mints in neither list, and their schedule stays as it
/// was.
///
/// # Size, and the boot
///
/// An entry is a `Pubkey` and its schedule, about 50 bytes: the ~3 000
/// unpriced mints of September 2026 stay well under a megabyte. A mint leaves
/// at its first price, and the rest is bounded by `token_metadata`, which only
/// grows as tokens are discovered.
///
/// It starts empty on every boot: the first tick asks every known mint, and the
/// waits build up again over the next half hour — a handful of extra asks per
/// dead mint, never a live one held back.
#[derive(Debug)]
pub struct UnpricedMints {
    deferred: HashMap<Pubkey, Deferral>,
    tick: Duration,
}

/// Where one unpriced mint stands.
#[derive(Debug, Clone, Copy)]
struct Deferral {
    /// Answers in a row without a price.
    misses: u32,
    /// When it is next worth asking.
    due_at: DateTime<Utc>,
}

impl UnpricedMints {
    /// Build the rule for a worker ticking every `tick_interval`.
    ///
    /// The cadence is the unit of the wait: a wait shorter than one tick would
    /// change nothing, since the mint is only looked at once per tick.
    pub fn new(tick_interval: core::time::Duration) -> Self {
        let tick = Duration::from_std(tick_interval).unwrap_or(UNPRICED_RETRY_MAX);

        Self {
            deferred: HashMap::new(),
            tick,
        }
    }

    /// Whether `mint` is worth asking the source about at `now`.
    pub fn is_due(&self, mint: &Pubkey, now: DateTime<Utc>) -> bool {
        self.deferred
            .get(mint)
            .is_none_or(|deferral| now >= deferral.due_at)
    }

    /// Take in what the source answered at `now`.
    ///
    /// `priced` are the mints it returned a price for, `unpriced` the ones it
    /// answered for without a price. A mint asked in a request that failed
    /// belongs in **neither**: see the type's documentation for why.
    pub fn record<'a>(
        &mut self,
        priced: impl IntoIterator<Item = &'a Pubkey>,
        unpriced: &[Pubkey],
        now: DateTime<Utc>,
    ) {
        for mint in priced {
            self.deferred.remove(mint);
        }

        for mint in unpriced {
            let misses = self
                .deferred
                .get(mint)
                .map_or(0, |deferral| deferral.misses)
                .saturating_add(1);

            self.deferred.insert(
                *mint,
                Deferral {
                    misses,
                    due_at: now + self.wait_after(misses),
                },
            );
        }
    }

    /// `cadence × 2^misses`, capped at [`UNPRICED_RETRY_MAX`].
    fn wait_after(&self, misses: u32) -> Duration {
        // Any cadence of a second or more reaches the cap well before 2²⁰, so
        // clamping the exponent there changes no wait and keeps the shift and
        // the multiplication from overflowing.
        let factor = 1_i32 << misses.min(20);

        self.tick
            .checked_mul(factor)
            .map_or(UNPRICED_RETRY_MAX, |wait| wait.min(UNPRICED_RETRY_MAX))
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
