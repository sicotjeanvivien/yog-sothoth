use std::{collections::VecDeque, sync::Mutex};

use async_trait::async_trait;
use yog_core::{RepositoryError, domain::ActiveKey};

use super::*;

fn key(byte: u8) -> ApiKey {
    ApiKey::from_random([byte; 4], [byte; 32])
}

fn active(key: &ApiKey, client: &str) -> ActiveKey {
    ActiveKey {
        hash: key.hash(),
        client: ApiClient {
            id: 1,
            name: client.to_string(),
        },
    }
}

/// Each read answers the next scripted outcome.
struct Scripted(Mutex<VecDeque<RepositoryResult<Vec<ActiveKey>>>>);

#[async_trait]
impl ApiKeyLookup for Scripted {
    async fn active_keys(&self) -> RepositoryResult<Vec<ActiveKey>> {
        self.0.lock().unwrap().pop_front().expect("a scripted read")
    }
}

async fn ring_reading(reads: Vec<RepositoryResult<Vec<ActiveKey>>>) -> KeyRing {
    KeyRing::load(Arc::new(Scripted(Mutex::new(reads.into()))))
        .await
        .expect("the first read loads")
}

/// Mutation: `client_of` keyed on anything but the key's hash.
#[tokio::test]
async fn a_key_of_the_ring_names_its_client_and_another_names_none() {
    let ring = ring_reading(vec![Ok(vec![active(&key(1), "dashboard")])]).await;

    assert_eq!(
        ring.client_of(&key(1)).map(|c| c.name).as_deref(),
        Some("dashboard")
    );
    assert_eq!(ring.client_of(&key(2)), None);
}

/// Mutation: `reload` reading without replacing what the ring holds.
#[tokio::test]
async fn a_reload_drops_a_revoked_key() {
    let ring = ring_reading(vec![Ok(vec![active(&key(1), "dashboard")]), Ok(vec![])]).await;

    assert_eq!(ring.reload().await, ReloadOutcome::Reloaded);

    assert_eq!(
        ring.client_of(&key(1)),
        None,
        "revoked between the two reads"
    );
}

/// Mutation: a failed read emptying the ring, which would refuse every client.
#[tokio::test]
async fn a_failed_reload_keeps_the_keys_held() {
    let ring = ring_reading(vec![
        Ok(vec![active(&key(1), "dashboard")]),
        Err(RepositoryError::Backend("down".to_string())),
    ])
    .await;

    assert_eq!(ring.reload().await, ReloadOutcome::Failed);

    assert!(
        ring.client_of(&key(1)).is_some(),
        "the ring kept what it held"
    );
}
