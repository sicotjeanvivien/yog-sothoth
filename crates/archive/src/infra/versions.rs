//! The server's versions, read over a connection opened for each run.

use async_trait::async_trait;
use yog_bootstrap::SecretUrl;
use yog_persistence::{Database, PgServerInfo, ServerVersions};

use crate::application::VersionSource;

/// Reads the server's versions over a connection opened for this run and closed after it.
/// ⚠️ Not once at startup: a refusing database would stop the process before the heartbeat could
/// say anything, a silent crash loop under `restart: unless-stopped`.
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
