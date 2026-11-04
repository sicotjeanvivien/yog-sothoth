use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// How long a hole under an id already read is waited for before it is passed over.
/// ⚠️ Ids are taken at insert, not at commit: a slower insert lands below an id already read.
const GAP_SETTLE: Duration = Duration::from_secs(10);

/// Where a poller stands in its table: every id up to `floor` is settled, and those above it
/// already delivered are kept, with when they were read, so that a hole filled later is
/// delivered without repeating them.
#[derive(Debug)]
pub(super) struct LiveWatermark {
    floor: i64,
    delivered: BTreeMap<i64, Instant>,
}

impl LiveWatermark {
    pub(super) fn anchored(tip: i64) -> Self {
        Self {
            floor: tip,
            delivered: BTreeMap::new(),
        }
    }

    pub(super) fn floor(&self) -> i64 {
        self.floor
    }

    /// The records not delivered yet, now counted as delivered, read at `now`.
    pub(super) fn fresh<R>(
        &mut self,
        records: Vec<R>,
        id_of: impl Fn(&R) -> i64,
        now: Instant,
    ) -> Vec<R> {
        records
            .into_iter()
            .filter(|record| {
                let id = id_of(record);
                id > self.floor && self.delivered.insert(id, now).is_none()
            })
            .collect()
    }

    /// Moves `floor` over the ids delivered without a hole, and over a hole under an id read
    /// `GAP_SETTLE` ago: each hole is dated by the id above it, not by when the floor reached it.
    pub(super) fn settle(&mut self, now: Instant) {
        loop {
            while self
                .delivered
                .first_key_value()
                .is_some_and(|(&id, _)| id == self.floor + 1)
            {
                self.delivered.pop_first();
                self.floor += 1;
            }
            let Some((&next, &read_at)) = self.delivered.first_key_value() else {
                return;
            };
            if now.duration_since(read_at) < GAP_SETTLE {
                return;
            }
            self.floor = next - 1;
        }
    }
}

#[cfg(test)]
#[path = "watermark_tests.rs"]
mod tests;
