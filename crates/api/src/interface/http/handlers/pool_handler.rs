use std::{collections::HashMap, sync::Arc};
use tracing::error;
use yog_core::domain::PoolRepository;

use crate::interface::http::dto::request::pagination_query::{PaginationQuery, encode_cursor};
use crate::interface::http::dto::response::page_response::PageResponse;
use crate::interface::http::dto::response::pool_response::PoolResponse;
use crate::interface::{HttpError, HttpRequest, HttpResponse, StatusCode};

#[derive(Clone)]
pub(crate) struct PoolHandler {
    pool_repository: Arc<dyn PoolRepository>,
}

impl PoolHandler {
    pub(crate) fn new(pool_repository: Arc<dyn PoolRepository>) -> Self {
        Self { pool_repository }
    }

    /// `GET /api/pools[?cursor=...&limit=...]`
    ///
    /// Returns a paginated list of discovered pools, ordered by
    /// `first_seen_at DESC`. The cursor is opaque — callers pass back
    /// the `next_cursor` from the previous response without
    /// interpreting it.
    pub(crate) async fn list(&self, request: HttpRequest) -> Result<HttpResponse, HttpResponse> {
        let query = PaginationQuery::from_params(&request.params)?;

        let page = self
            .pool_repository
            .find_paginated(query.cursor, query.limit)
            .await
            .map_err(|e| {
                error!(error = ?e, "failed to fetch pools");
                HttpError::InternalServerError("failed to fetch pools".to_string())
            })?;

        let items: Vec<PoolResponse> = page.items.into_iter().map(PoolResponse::from).collect();

        let next_cursor = match page.next_cursor {
            Some(ref c) => Some(encode_cursor(c)?),
            None => None,
        };

        let body = PageResponse { items, next_cursor };
        let json = serde_json::to_string(&body)?;

        Ok(HttpResponse::new(
            StatusCode::OK,
            json_response_headers(),
            Some(json),
        ))
    }
}

/// Standard headers for a JSON response.
///
/// Mirrors `MainHandler::build_header` but kept local to avoid coupling
/// across handlers; we will factor a shared helper if a third handler
/// reuses the same set verbatim.
fn json_response_headers() -> HashMap<String, String> {
    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    headers.insert("X-Content-Type-Options".to_string(), "nosniff".to_string());
    headers.insert("X-Frame-Options".to_string(), "DENY".to_string());
    headers.insert("Access-Control-Allow-Origin".to_string(), "*".to_string());
    headers.insert(
        "Access-Control-Allow-Methods".to_string(),
        "GET, POST, PATCH, DELETE".to_string(),
    );
    headers.insert(
        "Access-Control-Allow-Headers".to_string(),
        "Content-Type".to_string(),
    );
    headers.insert("Cache-Control".to_string(), "no-store".to_string());
    headers
}
