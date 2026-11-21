/// The part a source plays, named in the heartbeat's failure rather than its provider: the
/// provider is chosen by a URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceRole {
    Metadata,
    PoolAccount,
    Price,
}

impl SourceRole {
    pub(super) const ALL: [SourceRole; 3] = [
        SourceRole::Metadata,
        SourceRole::PoolAccount,
        SourceRole::Price,
    ];

    pub(crate) fn as_label(self) -> &'static str {
        match self {
            SourceRole::Metadata => "metadata",
            SourceRole::PoolAccount => "pool_account",
            SourceRole::Price => "price",
        }
    }

    pub(super) fn index(self) -> usize {
        match self {
            SourceRole::Metadata => 0,
            SourceRole::PoolAccount => 1,
            SourceRole::Price => 2,
        }
    }
}
