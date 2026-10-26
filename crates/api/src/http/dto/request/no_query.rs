use serde::Deserialize;
use utoipa::IntoParams;

/// The query of a route that takes none: any key is refused.
#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
pub(crate) struct NoQuery {}
