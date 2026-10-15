use serde::Serialize;

/// Overall verdict — the only value clients should branch on.
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ReadinessStatusResponse {
    Ready,
    Unhealthy,
}
