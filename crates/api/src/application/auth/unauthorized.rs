/// Why a credential was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unauthorized {
    /// None presented where the route requires one.
    Missing,
    /// `Authorization` names another scheme than `Bearer`, or is not text.
    Scheme,
    /// The bearer is not the credential the route takes.
    Malformed,
    /// A credential the API does not hold: never issued, revoked, or a ticket already used.
    Unknown,
    /// A ticket presented past its lifetime.
    Expired,
}

impl Unauthorized {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Scheme => "scheme",
            Self::Malformed => "malformed",
            Self::Unknown => "unknown",
            Self::Expired => "expired",
        }
    }
}
