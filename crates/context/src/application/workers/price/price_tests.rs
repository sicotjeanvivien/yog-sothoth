//! Unit tests for `PriceWorker::run_one_cycle`, driven by three fakes: the
//! metadata repository, the price repository and the price source.

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
use crate::application::source::{FetchedPrice, PriceAnswer, PriceSource};
use crate::error::SourceError;

// ── Helpers ───────────────────────────────────────────────────────────

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("valid decimal literal")
}

fn answer(priced: Vec<FetchedPrice>, unpriced: Vec<Pubkey>) -> Result<PriceAnswer, SourceError> {
    Ok(PriceAnswer { priced, unpriced })
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
    /// Lists served one per call before falling back to `known`.
    known_per_call: Mutex<Vec<Vec<Pubkey>>>,
    list_known_error: Mutex<Option<RepositoryError>>,
}

impl FakeMetadataRepository {
    fn with_known(mints: Vec<Pubkey>) -> Self {
        Self {
            known: Mutex::new(mints),
            ..Self::default()
        }
    }

    /// One list per tick, the last one repeated after that.
    fn with_known_per_call(mut lists: Vec<Vec<Pubkey>>) -> Self {
        let last = lists.pop().unwrap_or_default();
        Self {
            known: Mutex::new(last),
            known_per_call: Mutex::new(lists),
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
        let mut per_call = self.known_per_call.lock().unwrap();
        if !per_call.is_empty() {
            return Ok(per_call.remove(0));
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
        // Recorded even on a forced failure: the test asserts the attempt.
        self.inserts.lock().unwrap().push(prices.to_vec());

        if let Some(err) = self.insert_error.lock().unwrap().take() {
            return Err(err);
        }
        Ok(())
    }
}

#[derive(Default)]
struct FakePriceSource {
    responses: Mutex<Vec<Result<PriceAnswer, SourceError>>>,
    calls: Mutex<Vec<Vec<Pubkey>>>,
}

impl FakePriceSource {
    /// Prices only: a mint without a price is in neither list, as if its
    /// request had failed.
    fn with_responses(responses: Vec<Result<Vec<FetchedPrice>, SourceError>>) -> Self {
        Self::with_answers(
            responses
                .into_iter()
                .map(|response| {
                    response.map(|priced| PriceAnswer {
                        priced,
                        unpriced: vec![],
                    })
                })
                .collect(),
        )
    }

    fn with_answers(responses: Vec<Result<PriceAnswer, SourceError>>) -> Self {
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
    async fn fetch_prices(
        &self,
        mints: &[Pubkey],
        _shutdown: &CancellationToken,
    ) -> Result<PriceAnswer, SourceError> {
        self.calls.lock().unwrap().push(mints.to_vec());
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            return Ok(PriceAnswer::default());
        }
        responses.remove(0)
    }
}

/// A source the stop reaches mid-fetch: it cancels the token, as the signal
/// would, and returns the one price it had received.
struct StoppedDuringFetch(FetchedPrice);

#[async_trait]
impl PriceSource for StoppedDuringFetch {
    async fn fetch_prices(
        &self,
        _mints: &[Pubkey],
        shutdown: &CancellationToken,
    ) -> Result<PriceAnswer, SourceError> {
        shutdown.cancel();
        answer(vec![self.0.clone()], vec![])
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn no_known_mints_skips_source_and_insert() {
    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![]));
    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(FakePriceSource::with_responses(vec![]));

    let mut worker = PriceWorker::new(
        metadata_repo,
        price_repo.clone(),
        source.clone(),
        std::time::Duration::from_secs(30),
    );

    worker.run_one_cycle(&CancellationToken::new()).await;

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

    let mut worker = PriceWorker::new(
        metadata_repo,
        price_repo.clone(),
        source.clone(),
        std::time::Duration::from_secs(30),
    );

    worker.run_one_cycle(&CancellationToken::new()).await;

    let calls = source.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0], vec![mint_a, mint_b, mint_c]);

    let inserts = price_repo.inserts();
    assert_eq!(inserts.len(), 1, "exactly one insert_batch call");
    let batch = &inserts[0];
    assert_eq!(batch.len(), 3);

    // Distinct values per field: catches a field swap in `TokenPrice`.
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
    // The source prices fewer mints than asked: only those are inserted.
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

    let mut worker = PriceWorker::new(
        metadata_repo,
        price_repo.clone(),
        source.clone(),
        std::time::Duration::from_secs(30),
    );

    worker.run_one_cycle(&CancellationToken::new()).await;

    assert_eq!(source.calls()[0].len(), 3);

    let inserts = price_repo.inserts();
    assert_eq!(inserts.len(), 1);
    let batch = &inserts[0];
    assert_eq!(batch.len(), 2);
    assert_eq!(batch[0].mint, mint_a);
    assert_eq!(batch[1].mint, mint_c);
}

#[tokio::test]
async fn unstorable_price_is_dropped_before_the_batch() {
    // 4e-19 is positive but stores as 0: the CHECK of migration 009 would abort
    // the whole batch. Mutation this is written against: a `price_usd > 0`
    // filter.
    let mint_a = pk(1);
    let mint_dust = pk(2);
    let mint_c = pk(3);

    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![
        mint_a, mint_dust, mint_c,
    ]));
    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(FakePriceSource::with_responses(vec![Ok(vec![
        priced(mint_a, "1.0"),
        priced(mint_dust, "0.0000000000000000004"),
        priced(mint_c, "3.0"),
    ])]));

    let mut worker = PriceWorker::new(
        metadata_repo,
        price_repo.clone(),
        source.clone(),
        std::time::Duration::from_secs(30),
    );

    worker.run_one_cycle(&CancellationToken::new()).await;

    // The healthy mints are still written.
    let inserts = price_repo.inserts();
    assert_eq!(inserts.len(), 1, "the batch must still be sent");
    let batch = &inserts[0];
    assert_eq!(
        batch.iter().map(|p| p.mint).collect::<Vec<_>>(),
        vec![mint_a, mint_c],
        "only storable prices reach the repository"
    );
}

#[tokio::test]
async fn a_tick_of_only_unstorable_prices_inserts_nothing() {
    // The empty check sits after the filter: no insert at all.
    let mint_dust = pk(1);

    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![mint_dust]));
    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(FakePriceSource::with_responses(vec![Ok(vec![priced(
        mint_dust,
        "0.0000000000000000001",
    )])]));

    let mut worker = PriceWorker::new(
        metadata_repo,
        price_repo.clone(),
        source.clone(),
        std::time::Duration::from_secs(30),
    );

    worker.run_one_cycle(&CancellationToken::new()).await;

    assert!(
        price_repo.inserts().is_empty(),
        "a tick that keeps nothing must not call insert_batch"
    );
}

#[tokio::test]
async fn an_overflowing_price_is_dropped_before_the_batch() {
    // The other end: 1e20 overflows `NUMERIC(38, 18)` (`22003`) and would abort
    // the batch just the same.
    let mint_a = pk(1);
    let mint_absurd = pk(2);
    let mint_c = pk(3);

    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![
        mint_a,
        mint_absurd,
        mint_c,
    ]));
    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(FakePriceSource::with_responses(vec![Ok(vec![
        priced(mint_a, "1.0"),
        priced(mint_absurd, "100000000000000000000"),
        priced(mint_c, "3.0"),
    ])]));

    let mut worker = PriceWorker::new(
        metadata_repo,
        price_repo.clone(),
        source.clone(),
        std::time::Duration::from_secs(30),
    );

    worker.run_one_cycle(&CancellationToken::new()).await;

    let inserts = price_repo.inserts();
    assert_eq!(inserts.len(), 1, "the batch must still be sent");
    assert_eq!(
        inserts[0].iter().map(|p| p.mint).collect::<Vec<_>>(),
        vec![mint_a, mint_c],
        "only prices the column can hold reach the repository"
    );
}

#[tokio::test]
async fn midpoint_price_is_kept() {
    // 5e-19 rounds away from zero and stores as 1e-18: it must be kept.
    let mint = pk(1);

    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![mint]));
    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(FakePriceSource::with_responses(vec![Ok(vec![priced(
        mint,
        "0.0000000000000000005",
    )])]));

    let mut worker = PriceWorker::new(
        metadata_repo,
        price_repo.clone(),
        source.clone(),
        std::time::Duration::from_secs(30),
    );

    worker.run_one_cycle(&CancellationToken::new()).await;

    let inserts = price_repo.inserts();
    assert_eq!(inserts.len(), 1);
    assert_eq!(inserts[0].len(), 1);
    assert_eq!(inserts[0][0].mint, mint);
}

#[tokio::test]
async fn list_known_mints_error_skips_cycle_silently() {
    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![pk(1)]));
    metadata_repo.fail_list_known_once(RepositoryError::Integrity("DB down".into()));

    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(FakePriceSource::with_responses(vec![]));

    let mut worker = PriceWorker::new(
        metadata_repo,
        price_repo.clone(),
        source.clone(),
        std::time::Duration::from_secs(30),
    );

    worker.run_one_cycle(&CancellationToken::new()).await; // must not panic

    assert!(source.calls().is_empty());
    assert!(price_repo.inserts().is_empty());
}

#[tokio::test]
async fn no_insert_when_no_chunk_yields_a_price() {
    // The source prices nothing: no insert is attempted.
    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![pk(1), pk(2)]));
    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(FakePriceSource::with_responses(vec![Ok(vec![])]));

    let mut worker = PriceWorker::new(
        metadata_repo,
        price_repo.clone(),
        source.clone(),
        std::time::Duration::from_secs(30),
    );

    worker.run_one_cycle(&CancellationToken::new()).await;

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

    let mut worker = PriceWorker::new(
        metadata_repo,
        price_repo.clone(),
        source.clone(),
        std::time::Duration::from_secs(30),
    );

    worker.run_one_cycle(&CancellationToken::new()).await; // must not panic — that's the assertion

    // The insert was attempted before failing.
    assert_eq!(price_repo.inserts().len(), 1);
}

#[tokio::test]
async fn a_price_that_repeats_is_written_once() {
    // Two ticks milliseconds apart, one unchanged price, one row: only the
    // value comparison can suppress the second, not the floor.
    let mint = pk(1);
    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![mint]));
    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(FakePriceSource::with_responses(vec![
        Ok(vec![priced(mint, "1.0")]),
        Ok(vec![priced(mint, "1.0")]),
    ]));

    let mut worker = PriceWorker::new(
        metadata_repo,
        price_repo.clone(),
        source.clone(),
        std::time::Duration::from_secs(30),
    );

    worker.run_one_cycle(&CancellationToken::new()).await;
    worker.run_one_cycle(&CancellationToken::new()).await;

    assert_eq!(
        source.calls().len(),
        2,
        "both ticks still ASK for the price"
    );
    let inserts = price_repo.inserts();
    assert_eq!(inserts.len(), 1, "only the first tick writes");
    assert_eq!(inserts[0][0].price_usd, dec("1.0"));
}

#[tokio::test]
async fn a_price_that_moved_is_written_on_the_next_tick() {
    // The counterpart: a worker that stopped writing would pass the test above.
    let mint = pk(1);
    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![mint]));
    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(FakePriceSource::with_responses(vec![
        Ok(vec![priced(mint, "1.0")]),
        Ok(vec![priced(mint, "1.5")]),
    ]));

    let mut worker = PriceWorker::new(
        metadata_repo,
        price_repo.clone(),
        source.clone(),
        std::time::Duration::from_secs(30),
    );

    worker.run_one_cycle(&CancellationToken::new()).await;
    worker.run_one_cycle(&CancellationToken::new()).await;

    let inserts = price_repo.inserts();
    assert_eq!(inserts.len(), 2);
    assert_eq!(inserts[1][0].price_usd, dec("1.5"));
}

#[tokio::test]
async fn a_failed_insert_leaves_nothing_remembered() {
    // `KeptPrices::record` after a successful insert only: a refused batch
    // must be retried at the next tick, not wait for the floor.
    let mint = pk(1);
    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![mint]));
    let price_repo = Arc::new(FakePriceRepository::default());
    price_repo.fail_insert_once(RepositoryError::Integrity("disk full".into()));

    let source = Arc::new(FakePriceSource::with_responses(vec![
        Ok(vec![priced(mint, "1.0")]),
        Ok(vec![priced(mint, "1.0")]),
    ]));

    let mut worker = PriceWorker::new(
        metadata_repo,
        price_repo.clone(),
        source.clone(),
        std::time::Duration::from_secs(30),
    );

    worker.run_one_cycle(&CancellationToken::new()).await; // insert fails
    worker.run_one_cycle(&CancellationToken::new()).await; // same price — must be retried, not skipped

    assert_eq!(
        price_repo.inserts().len(),
        2,
        "the price the first tick could not write must be written by the second"
    );
}

// ── Mints the source answered without a price ─────────────────────────
//
// The waits are tested in yog-core; here, what the worker feeds the rule. Two
// ticks milliseconds apart: a deferred mint is still waiting on the second.

#[tokio::test]
async fn a_mint_answered_without_a_price_is_not_asked_on_the_next_tick() {
    let (live, dead) = (pk(1), pk(2));
    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![live, dead]));
    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(FakePriceSource::with_answers(vec![
        answer(vec![priced(live, "1.0")], vec![dead]),
        answer(vec![priced(live, "1.0")], vec![]),
    ]));

    let mut worker = PriceWorker::new(
        metadata_repo,
        price_repo,
        source.clone(),
        std::time::Duration::from_secs(30),
    );

    worker.run_one_cycle(&CancellationToken::new()).await;
    worker.run_one_cycle(&CancellationToken::new()).await;

    let calls = source.calls();
    assert_eq!(
        calls[0],
        vec![live, dead],
        "the first tick asks for everything"
    );
    assert_eq!(
        calls[1],
        vec![live],
        "the mint the source had no price for waits its turn"
    );
}

#[tokio::test]
async fn a_mint_whose_request_failed_is_asked_on_the_next_tick() {
    // Nothing was said about `silent` (a chunk given up on 429): it must not
    // be held back.
    let (live, silent) = (pk(1), pk(2));
    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![live, silent]));
    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(FakePriceSource::with_answers(vec![
        answer(vec![priced(live, "1.0")], vec![]),
        answer(vec![priced(live, "1.0")], vec![]),
    ]));

    let mut worker = PriceWorker::new(
        metadata_repo,
        price_repo,
        source.clone(),
        std::time::Duration::from_secs(30),
    );

    worker.run_one_cycle(&CancellationToken::new()).await;
    worker.run_one_cycle(&CancellationToken::new()).await;

    assert_eq!(
        source.calls()[1],
        vec![live, silent],
        "a mint the source said nothing about keeps being asked"
    );
}

#[tokio::test]
async fn an_unstorable_price_resets_the_wait_like_any_price() {
    // The rule is fed before the storability filter.
    let mint = pk(1);
    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![mint]));
    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(FakePriceSource::with_answers(vec![
        answer(vec![priced(mint, "0.0000000000000000001")], vec![]),
        answer(vec![], vec![mint]),
    ]));

    let mut worker = PriceWorker::new(
        metadata_repo,
        price_repo.clone(),
        source.clone(),
        std::time::Duration::from_secs(30),
    );
    // A mint three misses deep, whose wait has already run out.
    let long_ago = Utc::now() - chrono::Duration::hours(1);
    for _ in 0..3 {
        worker.unpriced.record([], &[mint], long_ago);
    }

    worker.run_one_cycle(&CancellationToken::new()).await; // a price, too small for the column
    worker.run_one_cycle(&CancellationToken::new()).await; // then no price at all

    assert!(
        price_repo.inserts().is_empty(),
        "nothing storable was priced"
    );
    assert_eq!(source.calls().len(), 2, "the mint was due on both ticks");
    // Reset by the unstorable price, this is a first miss: one minute, not
    // the eight of a fourth.
    assert!(
        worker
            .unpriced
            .is_due(&mint, Utc::now() + chrono::Duration::seconds(61)),
        "an unstorable price must reset the wait like any other"
    );
}

/// Mutation this is written against: the cycle returning before the insert
/// once the token is cancelled.
#[tokio::test]
async fn a_stop_during_a_tick_writes_what_was_received() {
    let (answered, never_asked) = (pk(1), pk(2));
    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![
        answered,
        never_asked,
    ]));
    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(StoppedDuringFetch(priced(answered, "1.5")));

    let mut worker = PriceWorker::new(
        metadata_repo,
        price_repo.clone(),
        source,
        std::time::Duration::from_secs(30),
    );

    let shutdown = CancellationToken::new();
    worker.run_one_cycle(&shutdown).await;

    assert!(shutdown.is_cancelled(), "premise: the stop came mid-fetch");
    let inserts = price_repo.inserts();
    assert_eq!(inserts.len(), 1, "what was received is written");
    assert_eq!(
        inserts[0].iter().map(|p| p.mint).collect::<Vec<_>>(),
        vec![answered]
    );
}

// ── Coverage metrics ──────────────────────────────────────────────────
//
// Not `#[tokio::test]`: `with_local_recorder` installs the recorder on the
// current thread only, so the future runs on a current-thread runtime inside
// it.

use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshotter};

/// Drive one cycle under a thread-local recorder and return its snapshot.
fn snapshot_one_cycle(
    worker: PriceWorker,
) -> Vec<(
    metrics_util::CompositeKey,
    Option<::metrics::Unit>,
    Option<::metrics::SharedString>,
    DebugValue,
)> {
    snapshot_cycles(worker, 1)
}

/// Same, over `cycles` consecutive ticks of the same worker.
fn snapshot_cycles(
    mut worker: PriceWorker,
    cycles: usize,
) -> Vec<(
    metrics_util::CompositeKey,
    Option<::metrics::Unit>,
    Option<::metrics::SharedString>,
    DebugValue,
)> {
    let recorder = DebuggingRecorder::new();
    let snapshotter: Snapshotter = recorder.snapshotter();

    ::metrics::with_local_recorder(&recorder, || {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("current-thread runtime")
            .block_on(async {
                for _ in 0..cycles {
                    worker.run_one_cycle(&CancellationToken::new()).await;
                }
            });
    });

    // ⚠️ One snapshot only: `snapshot()` resets counters, so a second call
    // would read zeros.
    snapshotter.snapshot().into_vec()
}

fn value<'a>(
    snapshot: &'a [(
        metrics_util::CompositeKey,
        Option<::metrics::Unit>,
        Option<::metrics::SharedString>,
        DebugValue,
    )],
    name: &str,
) -> Option<&'a DebugValue> {
    snapshot
        .iter()
        .find(|(key, _, _, _)| key.key().name() == name)
        .map(|(_, _, _, v)| v)
}

/// The count of ticks recorded under `outcome`, if any was.
fn tick_outcome<'a>(
    snapshot: &'a [(
        metrics_util::CompositeKey,
        Option<::metrics::Unit>,
        Option<::metrics::SharedString>,
        DebugValue,
    )],
    outcome: &str,
) -> Option<&'a DebugValue> {
    snapshot.iter().find_map(|(key, _, _, v)| {
        (key.key().name() == "yog_context_price_tick_total"
            && key
                .key()
                .labels()
                .any(|l| l.key() == "outcome" && l.value() == outcome))
        .then_some(v)
    })
}

#[test]
fn partial_price_coverage_is_reported_by_the_two_gauges() {
    // Two of three mints priced: only the pair of gauges says 2/3.
    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![
        pk(1),
        pk(2),
        pk(3),
    ]));
    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(FakePriceSource::with_responses(vec![Ok(vec![
        priced(pk(1), "1.0"),
        priced(pk(2), "2.0"),
    ])]));

    let snapshot = snapshot_one_cycle(PriceWorker::new(
        metadata_repo,
        price_repo,
        source,
        std::time::Duration::from_secs(30),
    ));

    assert_eq!(
        value(&snapshot, "yog_context_price_known_mints"),
        Some(&DebugValue::Gauge(3.0.into())),
        "the denominator: every known mint"
    );
    assert_eq!(
        value(&snapshot, "yog_context_price_priced_mints"),
        Some(&DebugValue::Gauge(2.0.into())),
        "the numerator: the mints that came back with a price — without it the \
         coverage cannot be computed, and its degradation stays invisible"
    );
}

#[test]
fn a_tick_that_priced_nothing_reports_zero_and_its_outcome() {
    // The source prices nothing: the gauge drops to 0 and `no_prices` is
    // emitted.
    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![pk(1), pk(2)]));
    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(FakePriceSource::with_responses(vec![Ok(vec![])]));

    let snapshot = snapshot_one_cycle(PriceWorker::new(
        metadata_repo,
        price_repo,
        source,
        std::time::Duration::from_secs(30),
    ));

    assert_eq!(
        value(&snapshot, "yog_context_price_priced_mints"),
        Some(&DebugValue::Gauge(0.0.into())),
        "a gauge left at its last value would report yesterday's coverage as \
         today's — total loss of coverage must read as 0, not as silence"
    );
    assert!(
        tick_outcome(&snapshot, "no_prices").is_some(),
        "the `no_prices` outcome must be emitted: a tick that priced nothing \
         used to return without recording anything, so it was indistinguishable \
         from a tick that never ran"
    );
}

#[test]
fn a_tick_with_no_known_mints_zeroes_both_gauges() {
    // Cold start: both gauges at 0, or `priced / known` reads +Inf.
    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![]));
    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(FakePriceSource::with_responses(vec![]));

    let snapshot = snapshot_one_cycle(PriceWorker::new(
        metadata_repo,
        price_repo,
        source,
        std::time::Duration::from_secs(30),
    ));

    assert_eq!(
        value(&snapshot, "yog_context_price_known_mints"),
        Some(&DebugValue::Gauge(0.0.into()))
    );
    assert_eq!(
        value(&snapshot, "yog_context_price_priced_mints"),
        Some(&DebugValue::Gauge(0.0.into())),
        "both gauges must move together — a 0 denominator against a stale \
         numerator makes the coverage alert fire on nothing"
    );
}

#[test]
fn a_hard_source_error_also_zeroes_the_priced_gauge() {
    // A hard source error zeroes the numerator too.
    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![pk(1), pk(2)]));
    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(FakePriceSource::with_responses(vec![Err(
        SourceError::Http("boom".into()),
    )]));

    let snapshot = snapshot_one_cycle(PriceWorker::new(
        metadata_repo,
        price_repo,
        source,
        std::time::Duration::from_secs(30),
    ));

    assert_eq!(
        value(&snapshot, "yog_context_price_known_mints"),
        Some(&DebugValue::Gauge(2.0.into()))
    );
    assert_eq!(
        value(&snapshot, "yog_context_price_priced_mints"),
        Some(&DebugValue::Gauge(0.0.into())),
        "a source that answered nothing priced nothing — the coverage ratio \
         must say 0, not hold its last value"
    );
}

#[test]
fn a_tick_that_changed_nothing_is_not_a_tick_that_priced_nothing() {
    // `unchanged` (the normal case) must not be labelled `no_prices` (an
    // alarm), and a suppressed price still counts in the coverage.
    let mint = pk(1);
    let metadata_repo = Arc::new(FakeMetadataRepository::with_known(vec![mint]));
    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(FakePriceSource::with_responses(vec![
        Ok(vec![priced(mint, "1.0")]),
        Ok(vec![priced(mint, "1.0")]),
    ]));

    let snapshot = snapshot_cycles(
        PriceWorker::new(
            metadata_repo,
            price_repo,
            source,
            std::time::Duration::from_secs(30),
        ),
        2,
    );

    let outcomes: Vec<String> = snapshot
        .iter()
        .filter(|(key, _, _, _)| key.key().name() == "yog_context_price_tick_total")
        .filter_map(|(key, _, _, _)| {
            key.key()
                .labels()
                .find(|l| l.key() == "outcome")
                .map(|l| l.value().to_string())
        })
        .collect();

    assert!(
        outcomes.contains(&"unchanged".to_string()),
        "the second tick wrote nothing on purpose and must say so — outcomes seen: {outcomes:?}"
    );
    assert!(
        !outcomes.contains(&"no_prices".to_string()),
        "`no_prices` means the source valued nothing, which is not what happened \
         — outcomes seen: {outcomes:?}"
    );

    assert_eq!(
        value(&snapshot, "yog_context_price_unchanged_total"),
        Some(&DebugValue::Counter(1)),
        "the suppressed row must be countable, or the redundancy of the series \
         is only knowable by querying the database"
    );
    assert_eq!(
        value(&snapshot, "yog_context_price_priced_mints"),
        Some(&DebugValue::Gauge(1.0.into())),
        "coverage is still 1/1 after a tick that suppressed its only price — \
         moving `set_priced_mints` below the redundancy filter would read 0 here"
    );
}

#[test]
fn every_counter_of_the_readme_ratios_is_published_before_any_tick() {
    // The exporter emits nothing for a counter never incremented: on a fresh
    // process the README's ratios would return no data.
    let recorder = DebuggingRecorder::new();
    let snapshotter: Snapshotter = recorder.snapshotter();
    ::metrics::with_local_recorder(&recorder, PriceWorkerMetrics::register_descriptions);
    let snapshot = snapshotter.snapshot().into_vec();

    // All three: PromQL's `+` matches nothing when one side is missing.
    for name in [
        "yog_context_price_rejected_total",
        "yog_context_price_unchanged_total",
        "yog_context_price_inserted_total",
    ] {
        assert_eq!(
            value(&snapshot, name),
            Some(&DebugValue::Counter(0)),
            "`{name}` must be published at 0 before any tick runs"
        );
    }
}

#[test]
fn a_tick_that_asks_nothing_says_why_and_zeroes_its_gauges() {
    // The second tick finds only `dead`, waiting its turn: no call, no
    // `no_prices`, and the first tick's coverage does not stand. The known list
    // shrinks between the ticks — `token_metadata` never does — because that is
    // the only way to observe this defensive reset.
    let (live, dead) = (pk(1), pk(2));
    let metadata_repo = Arc::new(FakeMetadataRepository::with_known_per_call(vec![
        vec![live, dead],
        vec![dead],
    ]));
    let price_repo = Arc::new(FakePriceRepository::default());
    let source = Arc::new(FakePriceSource::with_answers(vec![answer(
        vec![priced(live, "1.0")],
        vec![dead],
    )]));

    let snapshot = snapshot_cycles(
        PriceWorker::new(
            metadata_repo,
            price_repo,
            source.clone(),
            std::time::Duration::from_secs(30),
        ),
        2,
    );

    assert_eq!(source.calls().len(), 1, "the second tick asks nothing");
    let ticks = |outcome: &str| tick_outcome(&snapshot, outcome);
    assert_eq!(ticks("ok"), Some(&DebugValue::Counter(1)));
    assert_eq!(
        ticks("nothing_due"),
        Some(&DebugValue::Counter(1)),
        "the tick says why it asked nothing"
    );
    assert_eq!(
        ticks("no_prices"),
        None,
        "asking nothing is not pricing nothing"
    );
    assert_eq!(
        value(&snapshot, "yog_context_price_known_mints"),
        Some(&DebugValue::Gauge(1.0.into()))
    );
    assert_eq!(
        value(&snapshot, "yog_context_price_requested_mints"),
        Some(&DebugValue::Gauge(0.0.into())),
        "known but not requested: the share still worth asking is readable"
    );
    assert_eq!(
        value(&snapshot, "yog_context_price_priced_mints"),
        Some(&DebugValue::Gauge(0.0.into())),
        "the first tick's coverage must not outlive it"
    );
}
