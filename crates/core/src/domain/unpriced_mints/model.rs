//! Which mints the price source answered without a price, and when to ask it
//! again.
//!
//! The source's own answer is the signal: deciding whether to ask again does
//! not need to know *why* a mint has no price, only that the source, asked, had
//! none.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use solana_pubkey::Pubkey;

/// The longest a mint the source answered without a price waits before it is
/// asked again. It bounds how long a mint that comes back to life goes
/// unvalued; a longer cap would save almost no request.
const UNPRICED_RETRY_MAX: Duration = Duration::minutes(15);

/// The mints the source last answered without a price, and when each is due
/// again.
///
/// Held by the price worker. A mint not in here — never asked, or asked in a
/// request that failed — is due.
///
/// After `n` answers in a row without a price, a mint waits `cadence × 2ⁿ`, up
/// to `UNPRICED_RETRY_MAX`: 1, 2, 4, 8, then 15 minutes at a 30 s cadence. The
/// first answer with a price removes it, whether or not the column can store
/// that price.
///
/// ⚠️ **Decided one tick early.** The worker looks at a mint once per tick, and
/// the answer arrives after its tick began: a wait counted from the answer
/// would land a whole tick late. Subtracting one tick lands it on time, or one
/// tick early when the request is very short — never late. A tick that
/// overruns its cadence widens the bound to the cap plus one cycle.
///
/// ⚠️ **Only an answer without a price counts.** A request that failed says
/// nothing about its mints; counting it would hold back mints that have a
/// price. The caller passes those in neither list.
///
/// Starts empty on every boot, so the first tick asks every known mint.
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
    /// Build the rule for a worker ticking every `tick_interval`, the unit of
    /// the wait.
    pub fn new(tick_interval: core::time::Duration) -> Self {
        let tick = Duration::from_std(tick_interval).unwrap_or(UNPRICED_RETRY_MAX);

        Self {
            deferred: HashMap::new(),
            tick,
        }
    }

    /// The longest a mint without a price goes unasked at this cadence: the
    /// cap, or the cadence when it is longer. Stated by the price worker at
    /// startup, since an operator cannot derive it from the configuration.
    pub fn asks_again_at_most_every(&self) -> Duration {
        UNPRICED_RETRY_MAX.max(self.tick)
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
    /// belongs in **neither**.
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

            // One tick early (see the type). Above the cap, this lands in the
            // past: every tick asks.
            self.deferred.insert(
                *mint,
                Deferral {
                    misses,
                    due_at: now + self.wait_after(misses) - self.tick,
                },
            );
        }
    }

    /// `cadence × 2^misses`, capped at [`UNPRICED_RETRY_MAX`].
    fn wait_after(&self, misses: u32) -> Duration {
        // Any cadence reaches the cap well before 2²⁰: the clamp only keeps
        // the shift and the multiplication from overflowing.
        let factor = 1_i32 << misses.min(20);

        self.tick
            .checked_mul(factor)
            .map_or(UNPRICED_RETRY_MAX, |wait| wait.min(UNPRICED_RETRY_MAX))
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
