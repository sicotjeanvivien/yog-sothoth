//! The ids of the live tokens already opened, until they expire: a token opens once.

use std::collections::HashMap;

/// Each id opened, and when its token expires.
#[derive(Debug, Default)]
pub(crate) struct Consumed {
    ids: HashMap<String, i64>,
}

impl Consumed {
    /// Record `id`, whose token expires at `expires`; `false` when it was already recorded.
    /// ⚠️ An expired id is forgotten here, so the registry holds no more than the live tokens.
    pub(crate) fn consume(&mut self, id: &str, expires: i64, now: i64) -> bool {
        self.ids.retain(|_, until| *until > now);
        if self.ids.contains_key(id) {
            return false;
        }
        self.ids.insert(id.to_string(), expires);
        true
    }
}

#[cfg(test)]
#[path = "consumed_tests.rs"]
mod tests;
