use super::{FilterDecision, LogFilter};
use crate::infra::rpc::RawLogEvent;

pub struct FailedTransactionFilter;

impl LogFilter for FailedTransactionFilter {
    fn name(&self) -> &'static str {
        "failed_tx"
    }

    fn accept(&self, event: &RawLogEvent) -> FilterDecision {
        if event.err.is_some() {
            FilterDecision::Reject {
                reason: "transaction failed on-chain",
            }
        } else {
            FilterDecision::Accept
        }
    }
}
