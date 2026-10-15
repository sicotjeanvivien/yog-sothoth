use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ComponentStatusResponse {
    Up,
    Down,
}
