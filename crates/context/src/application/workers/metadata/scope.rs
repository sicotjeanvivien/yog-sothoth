//! Which pools a metadata tick reads the mints of.

/// The recent pools at every tick, all of them at the hourly sweep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Scope {
    RecentPools,
    AllPools,
}

impl Scope {
    pub(super) fn as_label(self) -> &'static str {
        match self {
            Self::RecentPools => "recent_pools",
            Self::AllPools => "all_pools",
        }
    }
}
