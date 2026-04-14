mod application;
mod bootstrap;
mod config;
mod infra;

use config::Config;
use crate::{bootstrap::Daemon};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let config = Config::load();
    Daemon::new(config).await?.run().await
}