use serde::Deserialize;
use utoipa::IntoParams;

use super::{PageDirectionParam, PagePositionParam, SeverityParam};
use crate::http::dto::request::default_limit;

/// Query params for the signals, of every protocol or of the entity in the path: standard
/// cursor pagination plus an optional exact severity. No sort (the feed ordering is fixed by
/// contract) and no free-text search.
#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
pub(crate) struct SignalsParams {
    #[param(min_length = 1)]
    pub(super) cursor: Option<String>,
    #[serde(default)]
    #[param(inline)]
    pub(super) dir: PageDirectionParam,
    #[param(inline)]
    pub(super) position: Option<PagePositionParam>,
    #[param(inline)]
    pub(super) severity: Option<SeverityParam>,
    #[serde(default = "default_limit")]
    #[param(value_type = u8, minimum = 1, maximum = 200)]
    pub(super) limit: i64,
}
