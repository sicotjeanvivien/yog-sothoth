use super::{FilterDecision, LogFilter};
use crate::infra::rpc::RawLogEvent;

pub struct InvocationFilter;

impl LogFilter for InvocationFilter {
    fn name(&self) -> &'static str {
        "invocation"
    }

    fn accept(&self, event: &RawLogEvent) -> FilterDecision {
        let program_id = event.protocol.program_id().to_string();
        let marker = format!("Program {program_id} invoke");

        if event.logs.iter().any(|log| log.starts_with(&marker)) {
            FilterDecision::Accept
        } else {
            FilterDecision::Reject {
                reason: "program not invoked (ALT-only reference)",
            }
        }
    }
}
