use thiserror::Error;

/// What can be wrong with a configured header, on either ingestion path: one rule, one type.
/// ⚠️ No variant carries the value, only the header's name, which tells the operator the `.env` line.
#[derive(Debug, Error)]
pub(crate) enum CredentialError {
    #[error(
        "`{name}` is not a valid header name — it must be a token, e.g. `x-token`. \
         Check `<FUNCTION>_HEADER_NAME`"
    )]
    InvalidHeaderName { name: String },

    #[error(
        "the value assembled for header `{name}` is not a valid header value — \
         check `<FUNCTION>_HEADER_VALUE` and `<FUNCTION>_KEY` (neither is printed here)"
    )]
    InvalidHeaderValue { name: String },
}
