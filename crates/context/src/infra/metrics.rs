//! Metrics shared by the `yog-context` providers: every chunk-level HTTP call records one
//! `call_total` and one `call_duration_seconds`, labelled by provider and outcome; an entry of an
//! answer that cannot be read records one `dropped_total`, labelled by provider and reason.

use metrics::{counter, describe_counter, describe_histogram, histogram};

const CALL_TOTAL: &str = "yog_context_provider_call_total";
const CALL_DURATION: &str = "yog_context_provider_call_duration_seconds";
const DROPPED_TOTAL: &str = "yog_context_provider_dropped_total";

pub(crate) struct ProviderMetrics;

impl ProviderMetrics {
    pub(crate) fn register_descriptions() {
        describe_counter!(
            CALL_TOTAL,
            "Provider HTTP calls (one per chunk). Labels: provider=helius_das|jupiter|solana_account, outcome=ok|http|rate_limited|decode"
        );
        describe_histogram!(
            CALL_DURATION,
            "Duration of a single provider HTTP call in seconds. Labels: provider, outcome"
        );
        describe_counter!(
            DROPPED_TOTAL,
            "Answer entries dropped as unreadable. Labels: provider=solana_account, reason=unreadable_owner|unreadable_data|length_mismatch"
        );
    }

    pub(crate) fn record_call(provider: &'static str, outcome: &'static str, seconds: f64) {
        counter!(
            CALL_TOTAL,
            "provider" => provider,
            "outcome" => outcome,
        )
        .increment(1);
        histogram!(
            CALL_DURATION,
            "provider" => provider,
            "outcome" => outcome,
        )
        .record(seconds);
    }

    pub(crate) fn record_dropped(provider: &'static str, reason: &'static str) {
        counter!(
            DROPPED_TOTAL,
            "provider" => provider,
            "reason" => reason,
        )
        .increment(1);
    }
}
