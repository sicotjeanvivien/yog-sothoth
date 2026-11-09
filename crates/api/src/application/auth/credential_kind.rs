/// The credential a route takes: what its refusal names, and the `credential` it is counted under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CredentialKind {
    ApiKey,
    Ticket,
}

impl CredentialKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ApiKey => "key",
            Self::Ticket => "ticket",
        }
    }
}
