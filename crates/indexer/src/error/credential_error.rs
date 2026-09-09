use thiserror::Error;

/// What can be wrong with a configured header, on either ingestion path.
///
/// One type for both because it is one rule: the header is validated the same
/// way whoever sends it, and a rule written twice is a rule that holds at one
/// site out of two.
///
/// ⚠️ **No variant carries the value**, and the second one is a word away from
/// doing so: what is wrong with it is the value, and quoting it is the natural
/// way to say that. The name is quoted instead — it is what tells an operator
/// which line of the `.env` to look at, and it is not a secret.
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
