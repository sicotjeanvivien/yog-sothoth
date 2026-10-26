use serde::Deserialize;

/// The query of a route that takes none: any key is refused.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NoQuery {}
