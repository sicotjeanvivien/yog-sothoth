//! What a run calls outside the process and outside the database layer: the
//! bucket, and the per-run read of the server's versions. The heartbeat is
//! `yog_bootstrap`'s ([`yog_bootstrap::Heartbeat`]), shared with `yog-signals`.
//! The dump itself is `yog_persistence`'s ([`yog_persistence::PgTools`]). The
//! run, which decides what to call and what each outcome signals, is
//! [`crate::application`].

mod store;
mod versions;

pub(crate) use store::{StoreSettings, open_store};
pub(crate) use versions::PgVersions;
