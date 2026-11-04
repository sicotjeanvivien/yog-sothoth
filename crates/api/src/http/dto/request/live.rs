//! Request DTO for `/api/live` and `/api/live/<entities>/{id}`: the topics of one connection.

use serde::Deserialize;
use utoipa::IntoParams;
use yog_core::domain::Protocol;

use crate::http::error::ApiError;

/// A collection the live stream carries: its name on the wire is its topic and its event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum LiveTopic {
    Signals,
}

impl LiveTopic {
    const SERVED: [Self; 1] = [Self::Signals];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Signals => "signals",
        }
    }
}

/// The query of a live connection.
#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
pub(crate) struct LiveParams {
    /// The collections to receive, comma-separated: `signals`.
    #[param(min_length = 1, example = "signals")]
    pub(super) topics: String,
}

/// One connection's topics, each once, and the entity it is restricted to.
#[derive(Debug)]
pub(crate) struct LiveRequest {
    pub(crate) topics: Vec<LiveTopic>,
    pub(crate) protocol: Option<Protocol>,
}

impl LiveRequest {
    /// ⚠️ A topic the stream does not serve is a 400, never ignored: the connection would open
    /// and stay silent, which a client cannot tell from a quiet feed.
    pub(crate) fn parse(params: LiveParams, protocol: Option<Protocol>) -> Result<Self, ApiError> {
        let raw = super::non_empty("topics", &params.topics)?;
        let mut topics = Vec::new();
        for name in raw.split(',') {
            let topic = LiveTopic::SERVED
                .into_iter()
                .find(|topic| topic.name() == name)
                .ok_or_else(|| ApiError::BadRequest(not_served(name)))?;
            if topics.contains(&topic) {
                return Err(ApiError::BadRequest(format!(
                    "`topics` names `{name}` twice"
                )));
            }
            topics.push(topic);
        }
        Ok(Self { topics, protocol })
    }
}

fn not_served(name: &str) -> String {
    let served: Vec<&str> = LiveTopic::SERVED.iter().map(|t| t.name()).collect();
    format!(
        "`topics`: `{name}` is not streamed; served: {}",
        served.join(", ")
    )
}
