//! Request DTO for `GET /api/signals`.

use solana_pubkey::Pubkey;
use yog_core::domain::{Severity, SignalCursor};
use yog_core::{PageDirection, PagePosition};

use crate::application::SignalListParams;
use crate::http::{cursor::decode_signal_cursor, error::ApiError};

mod page_direction_param;
mod page_position_param;
mod severity_param;
mod signals_query;

use page_direction_param::PageDirectionParam;
use page_position_param::PagePositionParam;
use severity_param::SeverityParam;
pub(crate) use signals_query::SignalsQuery;

#[derive(Debug)]
pub(crate) struct ListSignalsRequest {
    severity: Option<Severity>,
    pool: Option<Pubkey>,
    cursor: Option<SignalCursor>,
    direction: PageDirection,
    position: Option<PagePosition>,
    limit: i64,
}

impl ListSignalsRequest {
    pub(crate) fn parse(query: SignalsQuery) -> Result<Self, ApiError> {
        super::validate_limit(query.limit)?;

        let pool = query
            .pool
            .as_deref()
            .map(|raw| {
                super::non_empty("pool", raw).and_then(|raw| super::parse_address(raw, "pool"))
            })
            .transpose()?;

        // ⚠️ Decoded before the exclusivity check: an empty `cursor` beside a `position` is
        // refused for itself, not as a combination.
        let cursor = query
            .cursor
            .as_deref()
            .map(|raw| super::non_empty("cursor", raw).and_then(decode_signal_cursor))
            .transpose()?;
        validate_cursor_position_exclusive(cursor.is_some(), query.position.is_some())?;

        Ok(Self {
            severity: query.severity.map(Into::into),
            pool,
            cursor,
            direction: query.dir.into(),
            position: query.position.map(Into::into),
            limit: query.limit,
        })
    }

    pub(crate) fn into_params(self) -> SignalListParams {
        SignalListParams {
            severity: self.severity,
            pool: self.pool,
            cursor: self.cursor,
            direction: self.direction,
            position: self.position,
            limit: self.limit,
        }
    }
}

fn validate_cursor_position_exclusive(
    has_cursor: bool,
    has_position: bool,
) -> Result<(), ApiError> {
    if has_cursor && has_position {
        return Err(ApiError::BadRequest(
            "`position` cannot be combined with `cursor`".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/list_signals_tests.rs"]
mod tests;
