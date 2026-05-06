use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct ApiError {
    pub(crate) code: u16,
    pub(crate) message: String,
}
