//! Request DTO for `/api/live` and `/api/live/<entities>/{id}`: the topics of one connection.

use serde::Deserialize;
use utoipa::IntoParams;
use yog_core::domain::Protocol;

use crate::http::error::ApiError;

/// A collection the live stream carries: its name on the wire is its topic and its event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum LiveTopic {
    Signals,
    LiquidityChanges,
    PoolCreations,
    PositionChanges,
    PositionFeeClaims,
    ProtocolFeeClaims,
    RewardClaims,
    RewardChanges,
    PoolChanges,
}

impl LiveTopic {
    /// ⚠️ `swaps` is not one of them: dozens a second would drown both the page and the API.
    const SERVED: [Self; 9] = [
        Self::Signals,
        Self::LiquidityChanges,
        Self::PoolCreations,
        Self::PositionChanges,
        Self::PositionFeeClaims,
        Self::ProtocolFeeClaims,
        Self::RewardClaims,
        Self::RewardChanges,
        Self::PoolChanges,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Signals => "signals",
            Self::LiquidityChanges => "liquidity-changes",
            Self::PoolCreations => "pool-creations",
            Self::PositionChanges => "position-changes",
            Self::PositionFeeClaims => "position-fee-claims",
            Self::ProtocolFeeClaims => "protocol-fee-claims",
            Self::RewardClaims => "reward-claims",
            Self::RewardChanges => "reward-changes",
            Self::PoolChanges => "pool-changes",
        }
    }
}

/// The query of a live connection.
#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
pub(crate) struct LiveParams {
    /// The collections to receive, comma-separated: `signals` and the on-chain event
    /// collections but `swaps` (`liquidity-changes`, `pool-creations`…).
    #[param(min_length = 1, example = "signals,liquidity-changes")]
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
