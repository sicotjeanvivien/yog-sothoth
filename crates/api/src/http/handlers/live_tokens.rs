//! `POST /api/live-tokens` — a live token, against an API key, for one stream.

use axum::{Extension, Json, body::Bytes, extract::State};
use yog_core::domain::ApiClient;

use crate::application::{IssueError, Unauthorized, metrics::ApiMetrics};
use crate::http::{
    AppState,
    dto::{LiveTokenResponse, request::CreateLiveTokenBody},
    error::ApiError,
};

#[utoipa::path(
    post,
    path = "/api/live-tokens",
    responses((
        status = 200,
        description = "A live token for the stream named, to present once, within `expiresIn` \
                       seconds, as `Authorization: Bearer`. Without an API key: 401; past the \
                       10 000 tokens a client may issue in 30 s: 503.",
        body = LiveTokenResponse
    )),
    request_body = CreateLiveTokenBody,
    security(("bearer" = []))
)]
pub(crate) async fn create_live_token(
    State(state): State<AppState>,
    client: Option<Extension<ApiClient>>,
    body: Bytes,
) -> Result<Json<LiveTokenResponse>, ApiError> {
    let Some(Extension(client)) = client else {
        ApiMetrics::record_unauthorized(Unauthorized::Missing);
        return Err(ApiError::Unauthorized);
    };
    let scope = CreateLiveTokenBody::parse(&body)?;
    let token = state
        .live_tokens
        .issue(&client.name, &scope, chrono::Utc::now().timestamp())
        .map_err(|e| match e {
            IssueError::Full => {
                ApiError::Unavailable("too many live tokens issued to this client in 30 s")
            }
            IssueError::Random(e) => {
                ApiError::Internal(format!("the OS random source failed: {e}"))
            }
        })?;
    Ok(Json(LiveTokenResponse::bearer(token)))
}

#[cfg(test)]
#[path = "live_tokens_tests.rs"]
mod tests;
