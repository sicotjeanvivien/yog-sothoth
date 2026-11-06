use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use yog_core::{
    RepositoryResult,
    domain::{ActiveKey, ApiKeyLookup},
};

use super::*;

/// Answers the first read, then never again.
struct HangsAfterFirst(Mutex<bool>);

#[async_trait]
impl ApiKeyLookup for HangsAfterFirst {
    async fn active_keys(&self) -> RepositoryResult<Vec<ActiveKey>> {
        let first = std::mem::replace(&mut *self.0.lock().unwrap(), false);
        if !first {
            std::future::pending::<()>().await;
        }
        Ok(vec![])
    }
}

/// Mutation: the reload awaited outside the `select!`, so a read stuck on the pool holds the stop.
#[tokio::test]
async fn a_stop_reaches_a_reload_that_never_answers() {
    let ring = KeyRing::load(Arc::new(HangsAfterFirst(Mutex::new(true))))
        .await
        .expect("the first read answers");
    let shutdown = CancellationToken::new();
    let running = tokio::spawn(reload_key_ring(
        ring,
        Duration::from_millis(10),
        shutdown.clone(),
    ));

    tokio::time::sleep(Duration::from_millis(100)).await;
    shutdown.cancel();

    tokio::time::timeout(Duration::from_secs(5), running)
        .await
        .expect("the ring stops though its read never answers")
        .expect("no panic")
        .expect("infallible");
}
