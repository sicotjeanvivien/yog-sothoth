//! Metrics shared across `yog-context` providers (Helius DAS, Jupiter).
//!
//! Every chunk-level HTTP call records one `call_total` increment and
//! one `call_duration_seconds` observation, labelled by provider and
//! outcome. This gives back the per-chunk visibility that was lost when
//! chunking moved from the worker into the provider.

use metrics::{counter, describe_counter, describe_histogram, histogram};

const CALL_TOTAL: &str = "yog_context_provider_call_total";
const CALL_DURATION: &str = "yog_context_provider_call_duration_seconds";

pub(crate) struct ProviderMetrics;

impl ProviderMetrics {
    pub(crate) fn register_descriptions() {
        describe_counter!(
            CALL_TOTAL,
            "Provider HTTP calls (one per chunk). Labels: provider=helius_das|jupiter, outcome=ok|http|decode"
        );
        describe_histogram!(
            CALL_DURATION,
            "Duration of a single provider HTTP call in seconds. Labels: provider, outcome"
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
}
