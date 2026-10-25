use serde::Deserialize;
use utoipa::IntoParams;

use super::{PageDirectionParam, PagePositionParam, SeverityParam};
use crate::http::dto::request::default_limit;

/// Query params for the signals of one entity: those of `GET /api/signals` but the entity,
/// which is in the path.
#[derive(Debug, Deserialize, IntoParams)]
pub(crate) struct EntitySignalsParams {
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
