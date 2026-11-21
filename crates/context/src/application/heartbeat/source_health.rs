//! What each source answered since the last heartbeat; the reporter reads and clears the window.

use std::sync::Mutex;

use super::source_role::SourceRole;
use super::window_verdict::WindowVerdict;

/// One source's calls in the window.
#[derive(Debug, Default, Clone)]
struct Tally {
    calls: u64,
    successes: u64,
    last_error: Option<String>,
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
