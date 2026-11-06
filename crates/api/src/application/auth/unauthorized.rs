/// Why a credential was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unauthorized {
    /// No `Authorization` where one is required.
    Missing,
    /// `Authorization` names another scheme than `Bearer`, or is not text.
    Scheme,
    /// The bearer is not an API key.
    Malformed,
    /// An API key the ring does not hold: never issued, or revoked.
    Unknown,
    /// An API key presented to the live stream, which takes a live token only.
    KeyOnLive,
    /// A bearer that is no live token: not three parts, another header, an unreadable payload.
    TokenMalformed,
    /// A live token whose signature is not the API's.
    TokenSignature,
    TokenExpired,
    /// A live token already opened once.
    TokenUsed,
    /// A live token for another path, or for fewer topics than asked.
    TokenScope,
}

impl Unauthorized {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Scheme => "scheme",
            Self::Malformed => "malformed",
            Self::Unknown => "unknown",
            Self::KeyOnLive => "key_on_live",
            Self::TokenMalformed => "token_malformed",
            Self::TokenSignature => "token_signature",
            Self::TokenExpired => "token_expired",
            Self::TokenUsed => "token_used",
            Self::TokenScope => "token_scope",
        }
    }
}
