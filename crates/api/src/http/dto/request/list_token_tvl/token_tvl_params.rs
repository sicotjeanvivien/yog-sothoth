use serde::Deserialize;
use utoipa::IntoParams;

use super::SortParam;
use crate::http::dto::request::default_limit;

/// The order and forward-only cursor pagination; no window, a reading being an instant.
#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
pub(crate) struct TokenTvlParams {
    #[serde(default)]
    #[param(inline)]
    pub(super) sort: SortParam,
    #[param(min_length = 1)]
    pub(super) cursor: Option<String>,
    #[serde(default = "default_limit")]
    #[param(value_type = u8, minimum = 1, maximum = 200)]
    pub(super) limit: i64,
}
