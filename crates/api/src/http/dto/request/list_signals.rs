//! Request DTO for `GET /api/signals`.

use solana_pubkey::Pubkey;
use yog_core::domain::{Protocol, Severity, SignalCursor};
use yog_core::{PageDirection, PagePosition};

use crate::application::SignalListParams;
use crate::http::{cursor::decode_signal_cursor, error::ApiError};

mod entity_signals_query;
mod page_direction_param;
mod page_position_param;
mod severity_param;
mod signals_query;

pub(crate) use entity_signals_query::EntitySignalsQuery;
use page_direction_param::PageDirectionParam;
use page_position_param::PagePositionParam;
use severity_param::SeverityParam;
pub(crate) use signals_query::SignalsQuery;

#[derive(Debug)]
pub(crate) struct ListSignalsRequest {
    severity: Option<Severity>,
    protocol: Option<Protocol>,
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

        let cursor = parse_cursor(query.cursor.as_deref(), query.position.is_some())?;

        Ok(Self {
            severity: query.severity.map(Into::into),
            protocol: None,
            pool,
            cursor,
            direction: query.dir.into(),
            position: query.position.map(Into::into),
            limit: query.limit,
        })
    }

    /// The signals of one protocol, or of one pool, the entity of the path.
    pub(crate) fn parse_for_entity(
        query: EntitySignalsQuery,
        protocol: Option<Protocol>,
        pool: Option<Pubkey>,
    ) -> Result<Self, ApiError> {
        super::validate_limit(query.limit)?;
        let cursor = parse_cursor(query.cursor.as_deref(), query.position.is_some())?;
        Ok(Self {
            severity: query.severity.map(Into::into),
            protocol,
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
            protocol: self.protocol,
            pool: self.pool,
            cursor: self.cursor,
            direction: self.direction,
            position: self.position,
            limit: self.limit,
        }
    }
}

// ⚠️ Decoded before the exclusivity check: an empty `cursor` beside a `position` is
// refused for itself, not as a combination.
fn parse_cursor(raw: Option<&str>, has_position: bool) -> Result<Option<SignalCursor>, ApiError> {
    let cursor = raw
        .map(|raw| super::non_empty("cursor", raw).and_then(decode_signal_cursor))
        .transpose()?;
    validate_cursor_position_exclusive(cursor.is_some(), has_position)?;
    Ok(cursor)
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
