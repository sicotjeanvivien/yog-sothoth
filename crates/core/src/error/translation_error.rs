/// Errors a translation can produce.
///
/// These are reported via `ExtractionFailure::Translation` and never abort
/// the whole extraction.
#[derive(Debug, thiserror::Error)]
pub enum TranslationError {
    #[error("invalid {field} value: {value}")]
    InvalidEnum { field: &'static str, value: u8 },

    #[error("missing transferChecked context: {0}")]
    MissingTransferContext(String),
}
