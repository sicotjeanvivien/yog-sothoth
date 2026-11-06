//! Request DTO for `POST /api/live-tokens`: the stream a live token opens.

use serde::Deserialize;
use utoipa::ToSchema;

use super::{LiveParams, LiveRequest, parse_protocol};
use crate::application::LiveScope;
use crate::http::error::ApiError;

const LIVE: &str = "/api/live";
const PROTOCOL_LIVE: &str = "/api/live/protocols/";

/// The live stream a token is asked for, as it will be opened.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateLiveTokenBody {
    /// A path of the live stream: `/api/live`, or `/api/live/protocols/{protocol}`.
    #[schema(example = "/api/live/protocols/meteora_damm_v2")]
    path: String,
    /// The topics the stream will carry, as `topics` names them on `/api/live`.
    #[schema(example = json!(["signals", "liquidity-changes"]))]
    topics: Vec<String>,
}

impl CreateLiveTokenBody {
    /// The scope of `body`, each topic and the path checked as the live stream checks them.
    pub(crate) fn parse(body: &[u8]) -> Result<LiveScope, ApiError> {
        let body: Self = serde_json::from_slice(body)
            .map_err(|e| ApiError::BadRequest(format!("invalid body: {e}")))?;
        let protocol = match body.path.as_str() {
            LIVE => None,
            path => match path.strip_prefix(PROTOCOL_LIVE) {
                Some(protocol) => Some(parse_protocol(protocol)?),
                None => {
                    return Err(ApiError::BadRequest(format!(
                        "`path` is no path of the live stream: {path}"
                    )));
                }
            },
        };
        let request = LiveRequest::parse(
            LiveParams {
                topics: body.topics.join(","),
            },
            protocol,
        )?;
        Ok(LiveScope {
            path: protocol.map_or_else(|| LIVE.to_string(), |p| format!("{PROTOCOL_LIVE}{p}")),
            topics: request
                .topics
                .iter()
                .map(|t| t.name().to_string())
                .collect(),
        })
    }
}

#[cfg(test)]
#[path = "tests/create_live_token_tests.rs"]
mod tests;
