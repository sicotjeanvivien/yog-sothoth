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

pub trait LogFilter: Send + Sync {
    fn name(&self) -> &'static str;
    fn accept(&self, event: &RawLogEvent) -> FilterDecision;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::infra::rpc::RawLogEvent;

    use super::*;
    use solana_rpc_client_api::response::TransactionError;
    use yog_core::domain::Protocol;

    fn event_with_logs(logs: Vec<String>) -> RawLogEvent {
        RawLogEvent {
            protocol: Protocol::MeteoraDammV2,
            signature: "fake_sig".to_string(),
            logs,
            err: None,
        }
    }

    #[test]
    fn invocation_filter_accepts_top_level_invoke() {
        let program_id = Protocol::MeteoraDammV2.program_id().to_string();
        let event = event_with_logs(vec![format!("Program {program_id} invoke [1]")]);

        assert_eq!(InvocationFilter.accept(&event), FilterDecision::Accept);
    }

    #[test]
    fn invocation_filter_accepts_cpi_invoke() {
        let program_id = Protocol::MeteoraDammV2.program_id().to_string();
        let event = event_with_logs(vec![format!("Program {program_id} invoke [3]")]);

        assert_eq!(InvocationFilter.accept(&event), FilterDecision::Accept);
    }

    #[test]
    fn invocation_filter_rejects_alt_only() {
        // Program ID absent des logs — référencé uniquement via ALT.
        let event = event_with_logs(vec![
            "Program ComputeBudget111111111111111111111111111111 invoke [1]".to_string(),
        ]);

        assert!(matches!(
            InvocationFilter.accept(&event),
            FilterDecision::Reject { .. }
        ));
    }

    #[test]
    fn failed_tx_filter_rejects_when_err_present() {
        let mut event = event_with_logs(vec![]);
        event.err = Some(TransactionError::AccountNotFound);

        assert!(matches!(
            FailedTransactionFilter.accept(&event),
            FilterDecision::Reject { .. }
        ));
    }

    #[test]
    fn failed_tx_filter_accepts_when_err_none() {
        let event = event_with_logs(vec![]);
        assert_eq!(
            FailedTransactionFilter.accept(&event),
            FilterDecision::Accept
        );
    }
}
