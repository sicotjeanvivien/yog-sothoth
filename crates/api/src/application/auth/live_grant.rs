use crate::application::LiveScope;

/// What a live token opens, and until when: its client, its stream, its expiry in unix seconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LiveGrant {
    pub(super) client: String,
    pub(super) scope: LiveScope,
    pub(super) expires: i64,
}
