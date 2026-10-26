//! `/api/swaps/by-token`: a page of tokens by volume, over a window.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{TokenActivity, Window};

use super::token::EmbeddedTokenResponse;
use super::window::WindowResponse;
use crate::application::Measured;
use crate::http::cursor::encode_cursor_opt;
use crate::http::dto::schema::{Count, UnsignedDecimal};
use crate::http::error::ApiError;

/// ⚠️ A swap counts for both its tokens: the volumes add up to twice the total.
/// `poolHoursValued` of `poolHours` is the volume's coverage.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TokenActivityResponse {
    token: EmbeddedTokenResponse,
    #[schema(value_type = UnsignedDecimal)]
    volume_usd: Decimal,
    /// Distinct pools holding the token with a swap in the window.
    #[schema(value_type = Count)]
    pools: i64,
    #[schema(value_type = Count)]
    pool_hours: i64,
    #[schema(value_type = Count)]
    pool_hours_valued: i64,
}

/// The window envelope over a forward-only page: `prevCursor` is always `null`.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TokenActivityPageResponse {
    window: WindowResponse,
    from: Option<DateTime<Utc>>,
    as_of: Option<DateTime<Utc>>,
    covered_since: Option<DateTime<Utc>>,
    items: Vec<TokenActivityResponse>,
    next_cursor: Option<String>,
    prev_cursor: Option<String>,
    is_first: bool,
    is_last: bool,
}

impl TokenActivityPageResponse {
    pub(crate) fn new(
        window: Window,
        measured: Measured<yog_core::Page<TokenActivity>>,
    ) -> Result<Self, ApiError> {
        let period = measured.period;
        let page = measured.items;
        Ok(Self {
            window: window.into(),
            from: period.map(|s| s.from),
            as_of: period.map(|s| s.as_of),
            covered_since: period.map(|s| s.covered_since),
            next_cursor: encode_cursor_opt(page.next_cursor.as_ref())?,
            prev_cursor: encode_cursor_opt(page.prev_cursor.as_ref())?,
            is_first: page.is_first,
            is_last: page.is_last,
            items: page
                .items
                .into_iter()
                .map(|t| TokenActivityResponse {
                    token: EmbeddedTokenResponse::from_sources(Some(t.mint), t.metadata),
                    volume_usd: t.volume_usd,
                    pools: t.pools,
                    pool_hours: t.pool_hours,
                    pool_hours_valued: t.pool_hours_valued,
                })
                .collect(),
        })
    }
}
