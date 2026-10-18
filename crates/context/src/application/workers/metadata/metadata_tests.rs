//! Unit tests for `MetadataWorker::run_one_cycle`, where error absorption lives,
//! driven by `FakeRepository` and `FakeSource`; `run` is only a `select!` around it.

use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use solana_pubkey::Pubkey;

use yog_core::{
    RepositoryError, RepositoryResult,
    domain::{MetadataProvider, TokenMetadata, TokenMetadataRepository},
};

use super::*;
use crate::application::source::{FetchedMetadata, MetadataSource};
use crate::error::SourceError;

// ── Helpers ───────────────────────────────────────────────────────────

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

fn fetched(mint: Pubkey, symbol: &str, decimals: u8) -> FetchedMetadata {
    FetchedMetadata {
        mint,
        symbol: Some(symbol.to_string()),
        name: Some(format!("{symbol} Token")),
        decimals,
        logo_uri: None,
        metadata_provider: yog_core::domain::MetadataProvider::HeliusDas,
    }
}

// ── Fakes ─────────────────────────────────────────────────────────────

#[derive(Default)]
struct FakeRepository {
    /// Mints returned by either listing (when no error is set).
    missing: Mutex<Vec<Pubkey>>,
    /// If `Some`, the next listing returns this error, then `None`.
    list_missing_error: Mutex<Option<RepositoryError>>,
    /// Mints whose `upsert` must fail (returned in error order).
    upsert_failures: Mutex<Vec<Pubkey>>,
    /// Every `upsert` call recorded in order.
    upserts: Mutex<Vec<TokenMetadata>>,
    /// Every listing in order: its scope, and the window of a recent one.
    listings: Mutex<Vec<(Scope, Option<Duration>)>>,
}

impl FakeRepository {
    fn with_missing(mints: Vec<Pubkey>) -> Self {
        Self {
            missing: Mutex::new(mints),
            ..Self::default()
        }
    }

    fn fail_list_missing_once(&self, err: RepositoryError) {
        *self.list_missing_error.lock().unwrap() = Some(err);
    }

    fn fail_upsert_for(&self, mint: Pubkey) {
        self.upsert_failures.lock().unwrap().push(mint);
    }

    fn upserts(&self) -> Vec<TokenMetadata> {
        self.upserts.lock().unwrap().clone()
    }

    fn listings(&self) -> Vec<(Scope, Option<Duration>)> {
        self.listings.lock().unwrap().clone()
    }

    fn list(&self, scope: Scope, window: Option<Duration>) -> RepositoryResult<Vec<Pubkey>> {
        self.listings.lock().unwrap().push((scope, window));
        if let Some(err) = self.list_missing_error.lock().unwrap().take() {
            return Err(err);
        }
        Ok(self.missing.lock().unwrap().clone())
    }
}

#[async_trait]
impl TokenMetadataRepository for FakeRepository {
    async fn upsert(&self, metadata: &TokenMetadata) -> RepositoryResult<()> {
        // Recorded even on a forced failure: tests assert the worker kept iterating.
        self.upserts.lock().unwrap().push(metadata.clone());

        let mut failures = self.upsert_failures.lock().unwrap();
        if let Some(pos) = failures.iter().position(|m| *m == metadata.mint) {
            failures.remove(pos);
            return Err(RepositoryError::Integrity("forced upsert failure".into()));
        }
        Ok(())
    }

    async fn list_missing_mints_of_recent_pools(
        &self,
        window: Duration,
    ) -> RepositoryResult<Vec<Pubkey>> {
        self.list(Scope::RecentPools, Some(window))
    }

    async fn list_missing_mints_of_all_pools(&self) -> RepositoryResult<Vec<Pubkey>> {
        self.list(Scope::AllPools, None)
    }

    async fn list_price_candidates(&self, _window: Duration) -> RepositoryResult<Vec<Pubkey>> {
        Ok(vec![])
    }
}

#[derive(Default)]
struct FakeSource {
    /// Pre-configured responses, popped from the front on each call.
    responses: Mutex<Vec<Result<Vec<FetchedMetadata>, SourceError>>>,
    /// Mints received on each call, in order.
    calls: Mutex<Vec<Vec<Pubkey>>>,
}

impl FakeSource {
    fn with_responses(responses: Vec<Result<Vec<FetchedMetadata>, SourceError>>) -> Self {
        Self {
            responses: Mutex::new(responses),
            ..Self::default()
        }
    }

    fn calls(&self) -> Vec<Vec<Pubkey>> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl MetadataSource for FakeSource {
    async fn fetch_metadata(&self, mints: &[Pubkey]) -> Result<Vec<FetchedMetadata>, SourceError> {
        self.calls.lock().unwrap().push(mints.to_vec());
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            // A test that queued too few responses fails on `calls()`, not on silent empty data.
            return Ok(Vec::new());
        }
        responses.remove(0)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn no_missing_mints_skips_source_and_upsert() {
    let repository = Arc::new(FakeRepository::with_missing(vec![]));
    let source = Arc::new(FakeSource::with_responses(vec![]));

    let mut worker = MetadataWorker::new(
        repository.clone(),
        source.clone(),
        std::time::Duration::from_secs(10),
    );

    worker.run_one_cycle().await;

    assert!(source.calls().is_empty(), "source must not be called");
    assert!(repository.upserts().is_empty(), "no upsert expected");
}

#[tokio::test]
async fn enriches_all_mints_in_single_chunk() {
    let mint_a = pk(1);
    let mint_b = pk(2);
    let mint_c = pk(3);

    let repository = Arc::new(FakeRepository::with_missing(vec![mint_a, mint_b, mint_c]));
    let source = Arc::new(FakeSource::with_responses(vec![Ok(vec![
        fetched(mint_a, "AAA", 6),
        fetched(mint_b, "BBB", 9),
        fetched(mint_c, "CCC", 8),
    ])]));

    let mut worker = MetadataWorker::new(
        repository.clone(),
        source.clone(),
        std::time::Duration::from_secs(10),
    );

    worker.run_one_cycle().await;

    let calls = source.calls();
    assert_eq!(calls.len(), 1, "exactly one source call");
    assert_eq!(calls[0], vec![mint_a, mint_b, mint_c]);

    let upserts = repository.upserts();
    assert_eq!(upserts.len(), 3);
    assert_eq!(upserts[0].mint, mint_a);
    assert_eq!(upserts[0].symbol.as_deref(), Some("AAA"));
    assert_eq!(upserts[0].decimals, 6);
    assert_eq!(upserts[1].mint, mint_b);
    assert_eq!(upserts[1].decimals, 9);
    assert_eq!(upserts[2].mint, mint_c);

    // The worker stamps the same `now` on every item in a batch.
    let now_marker = upserts[0].fetched_at;
    assert_eq!(upserts[0].last_refresh_at, now_marker);
    assert!(upserts.iter().all(|m| m.fetched_at == now_marker));

    // Sanity: stamps are recent.
    let drift = Utc::now().signed_duration_since(now_marker);
    assert!(drift.num_seconds().abs() < 5);

    // Tag is set correctly.
    assert!(
        upserts
            .iter()
            .all(|m| m.metadata_provider == MetadataProvider::HeliusDas)
    );
}

#[tokio::test]
async fn upserts_only_what_source_returned() {
    // The source drops one of the three mints (no decimals): all three asked, two upserted.
    let mint_a = pk(1);
    let mint_b = pk(2);
    let mint_c = pk(3);

    let repository = Arc::new(FakeRepository::with_missing(vec![mint_a, mint_b, mint_c]));
    let source = Arc::new(FakeSource::with_responses(vec![Ok(vec![
        fetched(mint_a, "AAA", 6),
        fetched(mint_c, "CCC", 8),
    ])]));

    let mut worker = MetadataWorker::new(
        repository.clone(),
        source.clone(),
        std::time::Duration::from_secs(10),
    );

    worker.run_one_cycle().await;

    assert_eq!(source.calls()[0].len(), 3, "all three were requested");

    let upserts = repository.upserts();
    assert_eq!(upserts.len(), 2);
    assert_eq!(upserts[0].mint, mint_a);
    assert_eq!(upserts[1].mint, mint_c);
}

#[tokio::test]
async fn list_missing_error_skips_cycle_silently() {
    let repository = Arc::new(FakeRepository::with_missing(vec![pk(1)]));
    repository.fail_list_missing_once(RepositoryError::Integrity("DB down".into()));

    let source = Arc::new(FakeSource::with_responses(vec![]));

    let mut worker = MetadataWorker::new(
        repository.clone(),
        source.clone(),
        std::time::Duration::from_secs(10),
    );

    // Must NOT panic and must NOT call the source.
    worker.run_one_cycle().await;

    assert!(source.calls().is_empty());
    assert!(repository.upserts().is_empty());
}

#[tokio::test]
async fn source_error_skips_upserts_for_that_chunk() {
    let mint_a = pk(1);
    let repository = Arc::new(FakeRepository::with_missing(vec![mint_a]));
    let source = Arc::new(FakeSource::with_responses(vec![Err(SourceError::Http(
        "boom".into(),
    ))]));

    let mut worker = MetadataWorker::new(
        repository.clone(),
        source.clone(),
        std::time::Duration::from_secs(10),
    );

    worker.run_one_cycle().await;

    assert_eq!(source.calls().len(), 1, "source was called once");
    assert!(repository.upserts().is_empty(), "no upsert on source error",);
}

#[tokio::test]
async fn upsert_error_does_not_stop_the_batch() {
    let mint_a = pk(1);
    let mint_b = pk(2);
    let mint_c = pk(3);

    let repository = Arc::new(FakeRepository::with_missing(vec![mint_a, mint_b, mint_c]));
    repository.fail_upsert_for(mint_b); // middle item fails

    let source = Arc::new(FakeSource::with_responses(vec![Ok(vec![
        fetched(mint_a, "AAA", 6),
        fetched(mint_b, "BBB", 9),
        fetched(mint_c, "CCC", 8),
    ])]));

    let mut worker = MetadataWorker::new(
        repository.clone(),
        source.clone(),
        std::time::Duration::from_secs(10),
    );

    worker.run_one_cycle().await;

    // All three upserts attempted though the middle one failed: the resilience contract,
    // invisible at runtime.
    let upserts = repository.upserts();
    assert_eq!(upserts.len(), 3);
    assert_eq!(upserts[0].mint, mint_a);
    assert_eq!(upserts[1].mint, mint_b);
    assert_eq!(upserts[2].mint, mint_c);
}

fn idle_worker(repository: Arc<FakeRepository>, source: Arc<FakeSource>) -> MetadataWorker {
    MetadataWorker::new(repository, source, std::time::Duration::from_secs(10))
}

const RECENT: (Scope, Option<Duration>) = (Scope::RecentPools, Some(MISSING_MINT_DISCOVERY_WINDOW));
const SWEEP: (Scope, Option<Duration>) = (Scope::AllPools, None);

/// Against a first tick that reads only the recent pools, a sweep at every tick, or a window
/// other than `MISSING_MINT_DISCOVERY_WINDOW`.
#[tokio::test(start_paused = true)]
async fn the_first_tick_sweeps_every_pool_and_the_next_reads_the_recent_ones() {
    let repository = Arc::new(FakeRepository::with_missing(vec![]));
    let mut worker = idle_worker(repository.clone(), Arc::new(FakeSource::default()));

    worker.run_one_cycle().await;
    worker.run_one_cycle().await;

    assert_eq!(repository.listings(), vec![SWEEP, RECENT]);
}

/// Against a sweep due before `FULL_SWEEP_INTERVAL` has passed, or not due once it has.
#[tokio::test(start_paused = true)]
async fn the_sweep_comes_back_once_the_interval_has_passed() {
    let repository = Arc::new(FakeRepository::with_missing(vec![]));
    let mut worker = idle_worker(repository.clone(), Arc::new(FakeSource::default()));

    worker.run_one_cycle().await;
    tokio::time::advance(FULL_SWEEP_INTERVAL - std::time::Duration::from_secs(1)).await;
    worker.run_one_cycle().await;
    tokio::time::advance(std::time::Duration::from_secs(1)).await;
    worker.run_one_cycle().await;

    assert_eq!(repository.listings(), vec![SWEEP, RECENT, SWEEP]);
}

/// Against a sweep counted as done before its listing succeeded.
#[tokio::test(start_paused = true)]
async fn a_sweep_whose_listing_failed_is_done_again_at_the_next_tick() {
    let repository = Arc::new(FakeRepository::with_missing(vec![]));
    repository.fail_list_missing_once(RepositoryError::Integrity("DB down".into()));
    let mut worker = idle_worker(repository.clone(), Arc::new(FakeSource::default()));

    worker.run_one_cycle().await;
    worker.run_one_cycle().await;

    assert_eq!(repository.listings(), vec![SWEEP, SWEEP]);
}
