//! Request DTO for the on-chain event collections: `swaps`, `liquidity-changes` and
//! `pool-creations`, at the root, under a protocol and under a pool.

use chrono::{TimeDelta, Utc};
use serde::Deserialize;
use utoipa::IntoParams;
use yog_core::domain::{EventCursor, EventScope, Window};

use super::window_param::WindowParam;
use crate::http::{
    cursor::{EventCollection, decode_event_cursor},
    error::ApiError,
};

/// A window, ending at the first page, and forward-only cursor pagination.
#[derive(Debug, Deserialize, IntoParams)]
pub(crate) struct EventsParams {
    #[serde(default)]
    #[param(inline)]
    window: WindowParam,
    #[param(min_length = 1)]
    cursor: Option<String>,
    #[serde(default = "super::default_limit")]
    #[param(value_type = u8, minimum = 1, maximum = 200)]
    limit: i64,
}

/// How long past its widest window a cursor's start stays accepted, for pages read slowly.
const CURSOR_WINDOW_MARGIN: TimeDelta = TimeDelta::hours(1);

#[derive(Debug)]
pub(crate) struct ListEventsRequest {
    pub(crate) scope: EventScope,
    pub(crate) window: Window,
    pub(crate) cursor: Option<EventCursor>,
    pub(crate) limit: i64,
}

impl ListEventsRequest {
    /// `longest` is the widest window the list serves: a wider one is a 400.
    pub(crate) fn parse(
        params: EventsParams,
        collection: EventCollection,
        scope: EventScope,
        longest: Window,
    ) -> Result<Self, ApiError> {
        super::validate_limit(params.limit)?;
        let window: Window = params.window.into();
        if window.hours() > longest.hours() {
            return Err(ApiError::BadRequest(format!(
                "`window` beyond {} is not served for this list: read it under a pool",
                longest.as_str()
            )));
        }
        let cursor = params
            .cursor
            .as_deref()
            .map(|raw| {
                super::non_empty("cursor", raw)
                    .and_then(|raw| decode_event_cursor(raw, collection, scope))
            })
            .transpose()?;
        // ⚠️ The cursor's start is the client's to send: unchecked, a forged one reads past the
        // widest window the list serves.
        if let Some(c) = &cursor
            && (c.at < c.from || c.from < Utc::now() - longest.duration() - CURSOR_WINDOW_MARGIN)
        {
            return Err(ApiError::BadRequest(
                "invalid cursor: its window is not one this list serves".to_string(),
            ));
        }
        Ok(Self {
            scope,
            window,
            cursor,
            limit: params.limit,
        })
    }
}
