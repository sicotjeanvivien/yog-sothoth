//! The server's versions, read over a connection opened for each run.

use async_trait::async_trait;
use yog_bootstrap::SecretUrl;
use yog_persistence::{Database, PgServerInfo, ServerVersions};

use crate::archiver::VersionSource;

/// Reads the server's versions over a connection opened for this run and
/// closed after it.
///
/// Connecting once at startup made a refusing database — a wrong password, a
/// server that is down — stop the process before the heartbeat could say
/// anything. Under `restart: unless-stopped` that is a silent crash loop,
/// noticed only when the missing ping times out, hours later and without a
/// reason. Measured on 23 September 2026 with a wrong password: exit 1, no
/// signal. Connected here, the same refusal ends the run in `refused` and
/// sends the failure signal with the database's own words. One connection
/// every six hours costs nothing.
pub(crate) struct PgVersions {
    pub(crate) url: SecretUrl,
}

#[async_trait]
impl VersionSource for PgVersions {
    async fn server_versions(&self) -> Result<ServerVersions, String> {
        let database = Database::connect(self.url.expose()).await.map_err(|e| {
            format!(
                "cannot read the server's versions: cannot connect to the database: {}",
                self.url.scrub(&e.to_string())
            )
        })?;
        let versions = PgServerInfo::new(database.pool_owned())
            .server_versions()
            .await
            .map_err(|e| format!("cannot read the server's versions: {e}"));
        database.close().await;
        versions
    }
}
