//! Unit tests for `PriceWorker::run_one_cycle`.
//!
//! Same approach as the metadata worker tests: the infinite `run`
//! loop is left alone, `run_one_cycle` carries all the interesting
//! behaviour. Three fakes drive the worker — the metadata repository
//! (read-only: `list_known_mints`), the price repository (write:
//! `insert_batch`), and the price source.

use std::sync::Mutex;

use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use std::str::FromStr;

use yog_core::{
    RepositoryError, RepositoryResult,
    domain::{
        PriceProvider, TokenMetadata, TokenMetadataRepository, TokenPrice, TokenPriceRepository,
    },
};

use super::*;
use crate::error::SourceError;
use crate::source::{FetchedPrice, PriceSource};

// ── Helpers ───────────────────────────────────────────────────────────

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("valid decimal literal")
}

fn priced(mint: Pubkey, price: &str) -> FetchedPrice {
    FetchedPrice {
        mint,
        price_provider: PriceProvider::Jupiter,
        price_usd: dec(price),
    }
}

// ── Fakes ─────────────────────────────────────────────────────────────

#[derive(Default)]
struct FakeMetadataRepository {
    known: Mutex<Vec<Pubkey>>,
    list_known_error: Mutex<Option<RepositoryError>>,
}

impl FakeMetadataRepository {
    fn with_known(mints: Vec<Pubkey>) -> Self {
        Self {
            known: Mutex::new(mints),
            ..Self::default()
        }
    }

    fn fail_list_known_once(&self, err: RepositoryError) {
        *self.list_known_error.lock().unwrap() = Some(err);
    }
}

#[async_trait]
impl TokenMetadataRepository for FakeMetadataRepository {
    async fn list_known_mints(&self) -> RepositoryResult<Vec<Pubkey>> {
        if let Some(err) = self.list_known_error.lock().unwrap().take() {
            return Err(err);
        }
        Ok(self.known.lock().unwrap().clone())
    }

    async fn upsert(&self, _metadata: &TokenMetadata) -> RepositoryResult<()> {
        Ok(())
    }

    async fn list_missing_mints(&self) -> RepositoryResult<Vec<Pubkey>> {
        Ok(vec![])
    }
}

#[derive(Default)]
struct FakePriceRepository {
    inserts: Mutex<Vec<Vec<TokenPrice>>>,
    insert_error: Mutex<Option<RepositoryError>>,
}

impl FakePriceRepository {
    fn fail_insert_once(&self, err: RepositoryError) {
        *self.insert_error.lock().unwrap() = Some(err);
    }

    fn inserts(&self) -> Vec<Vec<TokenPrice>> {
        self.inserts.lock().unwrap().clone()
    }
}

#[async_trait]
impl TokenPriceRepository for FakePriceRepository {
    async fn insert_batch(&self, prices: &[TokenPrice]) -> RepositoryResult<()> {
        // Always record — even on forced failure — so we can assert
        // the worker did try to insert.
        self.inserts.lock().unwrap().push(prices.to_vec());

        if let Some(err) = self.insert_error.lock().unwrap().take() {
            return Err(err);
        }
        Ok(())
    }
}

#[derive(Default)]
struct FakePriceSource {
    responses: Mutex<Vec<Result<Vec<FetchedPrice>, SourceError>>>,
    calls: Mutex<Vec<Vec<Pubkey>>>,
}

impl FakePriceSource {
    fn with_responses(responses: Vec<Result<Vec<FetchedPrice>, SourceError>>) -> Self {
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
impl PriceSource for FakePriceSource {
    async fn fetch_prices(&self, mints: &[Pubkey]) -> Result<Vec<FetchedPrice>, SourceError> {
        self.calls.lock().unwrap().push(mints.to_vec());
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            return Ok(Vec::new());
        }
        responses.remove(0)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn no_known_mints_skips_source_and_insert() {
    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![]));
    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(FakePriceSource::with_responses(vec![]));

    let worker = PriceWorker::new(
        metadata_repo,
        price_repo.clone(),
        source.clone(),
        std::time::Duration::from_secs(30),
    );

    worker.run_one_cycle().await;

    assert!(source.calls().is_empty());
    assert!(price_repo.inserts().is_empty());
}

#[tokio::test]
async fn inserts_prices_for_all_priced_mints_with_uniform_timestamp() {
    let mint_a = pk(1);
    let mint_b = pk(2);
    let mint_c = pk(3);

    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![
        mint_a, mint_b, mint_c,
    ]));
    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(FakePriceSource::with_responses(vec![Ok(vec![
        priced(mint_a, "1.0"),
        priced(mint_b, "0.999"),
        priced(mint_c, "42.5"),
    ])]));

    let worker = PriceWorker::new(
        metadata_repo,
        price_repo.clone(),
        source.clone(),
        std::time::Duration::from_secs(30),
    );

    worker.run_one_cycle().await;

    let calls = source.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0], vec![mint_a, mint_b, mint_c]);

    let inserts = price_repo.inserts();
    assert_eq!(inserts.len(), 1, "exactly one insert_batch call");
    let batch = &inserts[0];
    assert_eq!(batch.len(), 3);

    // Distinct values per field — catches a field swap in the
    // worker's construction of `TokenPrice`.
    assert_eq!(batch[0].mint, mint_a);
    assert_eq!(batch[0].price_usd, dec("1.0"));
    assert_eq!(batch[1].mint, mint_b);
    assert_eq!(batch[1].price_usd, dec("0.999"));
    assert_eq!(batch[2].mint, mint_c);
    assert_eq!(batch[2].price_usd, dec("42.5"));

    // Property: a single `now` is stamped on every row of a tick.
    let stamp = batch[0].fetched_at;
    assert!(batch.iter().all(|p| p.fetched_at == stamp));

    // Property: source is Jupiter, confidence is None.
    assert!(
        batch
            .iter()
            .all(|p| matches!(p.price_provider, PriceProvider::Jupiter))
    );
    assert!(batch.iter().all(|p| p.confidence.is_none()));

    // Sanity: timestamp is recent.
    let drift = Utc::now().signed_duration_since(stamp);
    assert!(drift.num_seconds().abs() < 5);
}

#[tokio::test]
async fn inserts_only_what_source_priced() {
    // Source returns fewer prices than requested (Jupiter cannot
    // price untraded mints). The worker must still call the source
    // with the full list, then only insert what came back.
    let mint_a = pk(1);
    let mint_b = pk(2);
    let mint_c = pk(3);

    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![
        mint_a, mint_b, mint_c,
    ]));
    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(FakePriceSource::with_responses(vec![Ok(vec![
        priced(mint_a, "1.0"),
        priced(mint_c, "3.0"),
    ])]));

    let worker = PriceWorker::new(
        metadata_repo,
        price_repo.clone(),
        source.clone(),
        std::time::Duration::from_secs(30),
    );

    worker.run_one_cycle().await;

    assert_eq!(source.calls()[0].len(), 3);

    let inserts = price_repo.inserts();
    assert_eq!(inserts.len(), 1);
    let batch = &inserts[0];
    assert_eq!(batch.len(), 2);
    assert_eq!(batch[0].mint, mint_a);
    assert_eq!(batch[1].mint, mint_c);
}

#[tokio::test]
async fn list_known_mints_error_skips_cycle_silently() {
    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![pk(1)]));
    metadata_repo.fail_list_known_once(RepositoryError::Integrity("DB down".into()));

    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(FakePriceSource::with_responses(vec![]));

    let worker = PriceWorker::new(
        metadata_repo,
        price_repo.clone(),
        source.clone(),
        std::time::Duration::from_secs(30),
    );

    worker.run_one_cycle().await; // must not panic

    assert!(source.calls().is_empty());
    assert!(price_repo.inserts().is_empty());
}

#[tokio::test]
async fn no_insert_when_no_chunk_yields_a_price() {
    // All known mints, source returns empty responses for all chunks.
    // The worker's `to_insert.is_empty()` guard must prevent the
    // insert from ever being called.
    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![pk(1), pk(2)]));
    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(FakePriceSource::with_responses(vec![Ok(vec![])]));

    let worker = PriceWorker::new(
        metadata_repo,
        price_repo.clone(),
        source.clone(),
        std::time::Duration::from_secs(30),
    );

    worker.run_one_cycle().await;

    assert_eq!(source.calls().len(), 1, "source was called");
    assert!(
        price_repo.inserts().is_empty(),
        "no insert when nothing was priced",
    );
}

#[tokio::test]
async fn insert_batch_error_does_not_panic() {
    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![pk(1)]));
    let price_repo = Arc::new(FakePriceRepository::default());
    price_repo.fail_insert_once(RepositoryError::Integrity("disk full".into()));

    let source = Arc::new(FakePriceSource::with_responses(vec![Ok(vec![priced(
        pk(1),
        "1.0",
    )])]));

    let worker = PriceWorker::new(
        metadata_repo,
        price_repo.clone(),
        source.clone(),
        std::time::Duration::from_secs(30),
    );

    worker.run_one_cycle().await; // must not panic — that's the assertion

    // Sanity: the insert WAS attempted before failing.
    assert_eq!(price_repo.inserts().len(), 1);
}
