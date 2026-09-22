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

    let mut worker = PriceWorker::new(
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

    let mut worker = PriceWorker::new(
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

    let mut worker = PriceWorker::new(
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
async fn unstorable_price_is_dropped_before_the_batch() {
    // 4e-19 is POSITIVE, and stores as exactly 0 in `NUMERIC(38, 18)`. It must
    // never reach `insert_batch`: migration 009's CHECK would refuse it, and
    // because the batch is one statement with `ON CONFLICT DO NOTHING` — which
    // does not cover check violations — that refusal would take the two healthy
    // mints down with it, every tick.
    //
    // A `price_usd > 0` filter passes this value through, so this test is what
    // stands between the rule and the naive version of it.
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

    worker.run_one_cycle().await;

    // The healthy mints are still written — the dust price is skipped, not
    // fatal.
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
    // The empty check now sits after the filter, so this must NOT reach the
    // repository at all — an insert of an empty batch would be a wasted
    // round-trip, and `inserts()` staying empty is what proves the ordering.
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

    worker.run_one_cycle().await;

    assert!(
        price_repo.inserts().is_empty(),
        "a tick that keeps nothing must not call insert_batch"
    );
}

#[tokio::test]
async fn an_overflowing_price_is_dropped_before_the_batch() {
    // The other end of the column. 1e20 has 20 integer digits and overflows
    // `NUMERIC(38, 18)` with `22003` — refused by the TYPE, so migration 009's
    // CHECK never even runs. `usd_price` comes off the Jupiter response
    // unvalidated, so nothing upstream bounds it either, and the abort would
    // take the two healthy mints with it exactly like a zero would.
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

    worker.run_one_cycle().await;

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
    // 5e-19 rounds AWAY from zero and stores as 1e-18 — the filter must not be
    // over-eager and throw away a price the column can actually hold.
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

    worker.run_one_cycle().await;

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

    let mut worker = PriceWorker::new(
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

    let mut worker = PriceWorker::new(
        metadata_repo,
        price_repo.clone(),
        source.clone(),
        std::time::Duration::from_secs(30),
    );

    worker.run_one_cycle().await; // must not panic — that's the assertion

    // Sanity: the insert WAS attempted before failing.
    assert_eq!(price_repo.inserts().len(), 1);
}

#[tokio::test]
async fn a_price_that_repeats_is_written_once() {
    // The whole change in one test: two ticks, one unchanged price, one row.
    // The two ticks are milliseconds apart, so the 10-minute floor cannot be
    // what suppresses the second — only the value comparison can.
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

    worker.run_one_cycle().await;
    worker.run_one_cycle().await;

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
    // The counterpart, and what keeps the test above from passing against a
    // worker that simply stopped writing after its first tick.
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

    worker.run_one_cycle().await;
    worker.run_one_cycle().await;

    let inserts = price_repo.inserts();
    assert_eq!(inserts.len(), 2);
    assert_eq!(inserts[1][0].price_usd, dec("1.5"));
}

#[tokio::test]
async fn a_failed_insert_leaves_nothing_remembered() {
    // The ordering guard. `KeptPrices::record` runs only after a successful
    // insert; recording before it would make the worker believe a row exists
    // that the database refused, and the real price would then wait for the
    // 10-minute floor — a gap nothing backfills.
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

    worker.run_one_cycle().await; // insert fails
    worker.run_one_cycle().await; // same price — must be retried, not skipped

    assert_eq!(
        price_repo.inserts().len(),
        2,
        "the price the first tick could not write must be written by the second"
    );
}

// ── Coverage metrics ──────────────────────────────────────────────────
//
// The gauges are the whole point of the price-coverage observability: a
// price coverage that degrades must be visible, and a tick that priced nothing
// must not look like a tick that never happened.
//
// Not `#[tokio::test]`: `with_local_recorder` installs the recorder on the
// *current thread* for the duration of a closure, so the future has to be
// driven inside it — hence the current-thread runtime. Same recipe as
// yog-indexer's persistor metrics test.

use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshotter};

/// Drive one cycle under a thread-local recorder and return its snapshot.
fn snapshot_one_cycle(
    worker: PriceWorker,
) -> Vec<(
    metrics_util::CompositeKey,
    Option<metrics::Unit>,
    Option<metrics::SharedString>,
    DebugValue,
)> {
    snapshot_cycles(worker, 1)
}

/// Same, over `cycles` consecutive ticks of the SAME worker — the only way to
/// observe anything that depends on what the previous tick kept.
fn snapshot_cycles(
    mut worker: PriceWorker,
    cycles: usize,
) -> Vec<(
    metrics_util::CompositeKey,
    Option<metrics::Unit>,
    Option<metrics::SharedString>,
    DebugValue,
)> {
    let recorder = DebuggingRecorder::new();
    let snapshotter: Snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("current-thread runtime")
            .block_on(async {
                for _ in 0..cycles {
                    worker.run_one_cycle().await;
                }
            });
    });

    // ONE snapshot, queried repeatedly: `Snapshotter::snapshot` is destructive
    // for counters (`swap(0)`), so a second call returns zeros and would
    // "prove" a metric that never fired.
    snapshotter.snapshot().into_vec()
}

fn value<'a>(
    snapshot: &'a [(
        metrics_util::CompositeKey,
        Option<metrics::Unit>,
        Option<metrics::SharedString>,
        DebugValue,
    )],
    name: &str,
) -> Option<&'a DebugValue> {
    snapshot
        .iter()
        .find(|(key, _, _, _)| key.key().name() == name)
        .map(|(_, _, _, v)| v)
}

#[test]
fn partial_price_coverage_is_reported_by_the_two_gauges() {
    // Three known mints, only two of which the source can price. The pair of
    // gauges is the only thing that says the coverage is 2/3 — the insert
    // count alone cannot, since it has no denominator.
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
        "the denominator: every mint we asked about"
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
    // The source answers, but prices nothing. Two things must happen, and
    // neither did before: the gauge drops to 0 rather than holding the previous
    // tick's value, and the `no_prices` outcome — declared in the label set
    // from the start — is actually emitted.
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
        snapshot.iter().any(|(key, _, _, _)| {
            key.key().name() == "yog_context_price_tick_total"
                && key
                    .key()
                    .labels()
                    .any(|l| l.key() == "outcome" && l.value() == "no_prices")
        }),
        "the `no_prices` outcome must be emitted: a tick that priced nothing \
         used to return without recording anything, so it was indistinguishable \
         from a tick that never ran"
    );
}

#[test]
fn a_tick_with_no_known_mints_zeroes_both_gauges() {
    // Found in review. `set_known_mints` runs before the empty-mints early
    // return but `set_priced_mints` did not, so on a cold start the denominator
    // dropped to 0 while the numerator held its previous value — and the
    // README's own alert expression `priced / known` reads +Inf.
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
    // The third early return. `set_known_mints` has already run by then, so
    // leaving the numerator untouched reports yesterday's coverage against
    // today's denominator — for as long as the source keeps failing.
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
    // Two outcomes that look alike from the outside and mean opposite things.
    // `no_prices` is an anomaly worth alerting on — the source valued nothing.
    // `unchanged` is the normal case once redundant rows are suppressed, and
    // after this change it is most ticks: labelling it `no_prices` would leave
    // that alert lit for ever.
    //
    // The gauge is the second half of the test. `priced_mints` answers "what
    // can be valued downstream", and a price suppressed as redundant still can
    // be — by the row that already carries it. Counting it after the
    // redundancy filter would read as a coverage collapse.
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
    // `describe_counter!` registers help text only: the Prometheus exporter
    // emits nothing for a counter never incremented. Both of these are
    // incremented deep inside a tick, after three early returns, so on a fresh
    // process — `token_metadata` still empty, every tick returning at
    // `no_work` — neither series would exist and the README's redundancy PromQL
    // would return no data, during exactly the window someone is watching a
    // deployment.
    let recorder = DebuggingRecorder::new();
    let snapshotter: Snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, PriceWorkerMetrics::register_descriptions);
    let snapshot = snapshotter.snapshot().into_vec();

    // All three, not just the two the redundancy rule touches: that ratio is
    // `unchanged / (unchanged + inserted)`, and PromQL's vector-to-vector `+`
    // matches nothing when one side is missing — publishing the numerator
    // alone would leave the query just as empty.
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
