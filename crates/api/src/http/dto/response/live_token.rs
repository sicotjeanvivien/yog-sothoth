//! Response DTO for `POST /api/live-tokens`: the fields of OAuth 2.0's client credentials grant,
//! named in the API's camelCase (`accessToken`, not `access_token`).

use serde::Serialize;
use utoipa::ToSchema;

use crate::application::LIVE_TOKEN_TTL_SECS;

/// A live token, to present once as `Authorization: Bearer` on the stream it names.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiveTokenResponse {
    access_token: String,
    /// Always `Bearer`.
    token_type: String,
    /// Seconds the token opens its stream for, from now.
    expires_in: u32,
}

impl LiveTokenResponse {
    pub(crate) fn bearer(access_token: String) -> Self {
        Self {
            access_token,
            token_type: "Bearer".to_string(),
            expires_in: LIVE_TOKEN_TTL_SECS,
        }
    }
}
