//! Request DTO for `GET /api/signals`.

use yog_core::domain::{Severity, SignalCursor};
use yog_core::{PageDirection, PagePosition};

use crate::application::SignalListParams;
use crate::http::{
    cursor::decode_signal_cursor,
    error::ApiError,
    query::{SignalsQuery, validate_cursor_position_exclusive, validate_limit},
};

#[derive(Debug)]
pub(crate) struct ListSignalsRequest {
    severity: Option<Severity>,
    cursor: Option<SignalCursor>,
    direction: PageDirection,
    position: Option<PagePosition>,
    limit: i64,
}

impl ListSignalsRequest {
    /// Validate the query into a single request value.
    ///
    /// Sort is intentionally not exposed by this endpoint — the feed is
    /// ordered by `(triggered_at DESC, id DESC)` by contract. `severity`
    /// arrives pre-validated: an unknown value already failed serde.
    pub(crate) fn parse(query: SignalsQuery) -> Result<Self, ApiError> {
        validate_limit(query.limit)?;
        validate_cursor_position_exclusive(query.cursor.is_some(), query.position.is_some())?;

        let cursor = match query.cursor.as_deref() {
            Some(raw) if !raw.is_empty() => Some(decode_signal_cursor(raw)?),
            _ => None,
        };

        Ok(Self {
            severity: query.severity.map(Into::into),
            cursor,
            direction: query.dir.into(),
            position: query.position.map(Into::into),
            limit: query.limit,
        })
    }

    pub(crate) fn into_params(self) -> SignalListParams {
        SignalListParams {
            severity: self.severity,
            cursor: self.cursor,
            direction: self.direction,
            position: self.position,
            limit: self.limit,
        }
    }
}

#[cfg(test)]
#[path = "tests/list_signals_tests.rs"]
mod tests;
