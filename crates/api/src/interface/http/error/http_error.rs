#[derive(Debug, thiserror::Error, PartialEq)]
pub(crate) enum HttpError {
    #[error("Method Not Found: {0}")]
    MethodNotFound(String),
    #[error("Param Not Found: {0}")]
    ParamNotFound(String),
    #[error("Bad Request: {0}")]
    BadRequest(String),
    #[error("Internal Server Error: {0}")]
    InternalServerError(String),
    #[error("Request Timed Out")]
    Timeout,
    #[error("Invalid Encoding")]
    InvalidEncoding,
    #[error("Malformed Request")]
    MalformedRequest,
    #[error("Io Error")]
    IoError,
}
