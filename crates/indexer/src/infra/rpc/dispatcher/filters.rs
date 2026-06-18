use crate::infra::rpc::RawLogEvent;

pub(crate) mod failed_transaction;
pub(crate) mod invocation;

pub(crate) use failed_transaction::FailedTransactionFilter;
pub(crate) use invocation::InvocationFilter;

/// Décision d'un filtre sur un événement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FilterDecision {
    Accept,
    Reject { reason: &'static str },
}

pub(crate) trait LogFilter: Send + Sync {
    fn name(&self) -> &'static str;
    fn accept(&self, event: &RawLogEvent) -> FilterDecision;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "filters_tests.rs"]
mod tests;
