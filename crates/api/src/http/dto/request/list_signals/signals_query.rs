use serde::Deserialize;
use utoipa::IntoParams;

use super::{PageDirectionParam, PagePositionParam, SeverityParam};

const DEFAULT_LIMIT: i64 = 50;
pub(crate) const MAX_LIMIT: i64 = 200;

pub(super) fn default_limit() -> i64 {
    DEFAULT_LIMIT
}

/// Query params for `GET /api/signals`: standard cursor pagination plus
/// optional exact severity and pool filters. No sort (the feed ordering
/// is fixed by contract) and no free-text search.
#[derive(Debug, Deserialize, IntoParams)]
pub(crate) struct SignalsQuery {
    pub(super) cursor: Option<String>,
    #[serde(default)]
    #[param(inline)]
    pub(super) dir: PageDirectionParam,
    #[param(inline)]
    pub(super) position: Option<PagePositionParam>,
    #[param(inline)]
    pub(super) severity: Option<SeverityParam>,
    pub(super) pool: Option<String>,
    #[serde(default = "default_limit")]
    #[param(value_type = u8, minimum = 1, maximum = 200)]
    pub(super) limit: i64,
}
