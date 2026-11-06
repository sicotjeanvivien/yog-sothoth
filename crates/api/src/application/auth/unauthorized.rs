/// Why a credential was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unauthorized {
    /// `Authorization` names another scheme than `Bearer`, or is not text.
    Scheme,
    /// The bearer is not an API key.
    Malformed,
    /// An API key the ring does not hold: never issued, or revoked.
    Unknown,
}

impl Unauthorized {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Scheme => "scheme",
            Self::Malformed => "malformed",
            Self::Unknown => "unknown",
        }
    }
}
