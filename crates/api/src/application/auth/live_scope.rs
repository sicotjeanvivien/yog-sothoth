/// What a live token opens: one path of the live stream, and the topics it may carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LiveScope {
    pub(crate) path: String,
    pub(crate) topics: Vec<String>,
}

impl LiveScope {
    /// Whether an opening of `asked` stays inside this scope: the same path, no topic of its own.
    pub(crate) fn covers(&self, asked: &LiveScope) -> bool {
        self.path == asked.path && asked.topics.iter().all(|t| self.topics.contains(t))
    }
}

#[cfg(test)]
#[path = "live_scope_tests.rs"]
mod tests;
