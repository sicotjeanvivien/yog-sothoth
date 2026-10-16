/// Errors a translation can produce, reported as `ExtractionFailure::Translation`.
#[derive(Debug, thiserror::Error)]
pub enum TranslationError {
    #[error("invalid {field} value: {value}")]
    InvalidEnum { field: &'static str, value: u8 },
}
