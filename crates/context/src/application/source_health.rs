//! What each source answered since the last heartbeat: the three workers record every call,
//! the heartbeat reporter reads and clears the window.

use std::sync::Mutex;

/// The part a source plays, named in the heartbeat's failure rather than its provider: the
/// provider is chosen by a URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceRole {
    Metadata,
    PoolAccount,
    Price,
}

impl SourceRole {
    const ALL: [SourceRole; 3] = [
        SourceRole::Metadata,
        SourceRole::PoolAccount,
        SourceRole::Price,
    ];

    pub(crate) fn as_label(self) -> &'static str {
        match self {
            SourceRole::Metadata => "metadata",
            SourceRole::PoolAccount => "pool_account",
            SourceRole::Price => "price",
        }
    }

    fn index(self) -> usize {
        match self {
            SourceRole::Metadata => 0,
            SourceRole::PoolAccount => 1,
            SourceRole::Price => 2,
        }
    }
}

/// One source's calls in the window.
#[derive(Debug, Default, Clone)]
struct Tally {
    calls: u64,
    successes: u64,
    last_error: Option<String>,
}

/// The verdict on a window: a source called without a single success fails it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum WindowVerdict {
    Healthy,
    /// Names each failing source, by role.
    Failing(String),
}

#[derive(Debug, Default)]
pub(crate) struct SourceHealth {
    window: Mutex<[Tally; 3]>,
}

impl SourceHealth {
    pub(crate) fn record_success(&self, role: SourceRole) {
        let mut window = self.lock();
        let tally = &mut window[role.index()];
        tally.calls += 1;
        tally.successes += 1;
    }

    pub(crate) fn record_failure(&self, role: SourceRole, error: String) {
        let mut window = self.lock();
        let tally = &mut window[role.index()];
        tally.calls += 1;
        tally.last_error = Some(error);
    }

    /// The verdict on the window since the last call, which starts a new one.
    pub(crate) fn take_window(&self) -> WindowVerdict {
        let window = std::mem::take(&mut *self.lock());
        let failing: Vec<String> = SourceRole::ALL
            .iter()
            .filter_map(|role| {
                let tally = &window[role.index()];
                (tally.calls > 0 && tally.successes == 0).then(|| failure_line(*role, tally))
            })
            .collect();
        if failing.is_empty() {
            WindowVerdict::Healthy
        } else {
            WindowVerdict::Failing(failing.join("; "))
        }
    }

    /// ⚠️ A poisoned lock still holds counts: a panic elsewhere must not silence the alarm.
    fn lock(&self) -> std::sync::MutexGuard<'_, [Tally; 3]> {
        self.window
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn failure_line(role: SourceRole, tally: &Tally) -> String {
    let last = tally.last_error.as_deref().unwrap_or_default();
    format!(
        "{}: {} calls, none succeeded, last error: {last}",
        role.as_label(),
        tally.calls
    )
}

#[cfg(test)]
#[path = "source_health_tests.rs"]
mod tests;
