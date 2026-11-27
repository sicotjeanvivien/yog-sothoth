//! Unit tests for `PoolAccountWorker::run_one_cycle` against fakes yielding raw accounts, so they
//! run through the real `core` decoder.

use super::*;
use async_trait::async_trait;
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use std::sync::Mutex;
use yog_core::RepositoryResult;
use yog_core::amm::damm_v2::BaseFeeKind;

use yog_core::domain::{
    FieldOutOfRange, MeteoraDammV2PoolAccountProperties, PoolAccountProperties,
    PoolAccountResolver, PoolRegistryProperties, PoolRepository, Protocol,
};

use crate::application::source::RawAccount;
use crate::error::SourceError;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

/// A cp-amm `Pool` account buffer; offsets mirror `core`'s DAMM v2 decoder, and drift fails both.
fn cp_amm_account(token_a: Pubkey, token_b: Pubkey) -> Vec<u8> {
    let mut bytes = vec![0u8; 1112];
    bytes[..8].copy_from_slice(&[0xf1, 0x9a, 0x6d, 0x04, 0x11, 0xb1, 0x6d, 0xbc]);
    bytes[8..16].copy_from_slice(&2_500_000u64.to_le_bytes()); // 25 bps
    bytes[16] = 0; // BaseFeeMode::FeeTimeSchedulerLinear
    bytes[22..24].copy_from_slice(&0u16.to_le_bytes()); // no periods → constant
    bytes[48] = 20; // protocol
    bytes[49] = 0; // padding_0 — decoded by nothing
    bytes[50] = 20; // referral
    bytes[56] = 1; // dynamic_fee.initialized
    bytes[168..200].copy_from_slice(token_a.as_ref());
    bytes[200..232].copy_from_slice(token_b.as_ref());
    bytes
}

/// A DLMM `LbPair` buffer the decoder accepts: its discriminator, the rest zero.
fn lb_pair_account() -> Vec<u8> {
    let mut bytes = vec![0u8; 904];
    bytes[..8].copy_from_slice(&[0x21, 0x0b, 0x31, 0x62, 0xb5, 0x65, 0xb1, 0x0d]);
    bytes
}

fn expected_properties() -> PoolAccountProperties {
    PoolAccountProperties::MeteoraDammV2(MeteoraDammV2PoolAccountProperties {
        protocol_fee_percent: 20,
        referral_fee_percent: 20,
        base_fee_kind: Some(BaseFeeKind::Constant),
        has_dynamic_fee: true,
        // A constant fee: no decay curve, `number_of_period == 0`.
        fee_scheduler: None,
        sqrt_min_price: Some(0),
        sqrt_max_price: Some(0),
        sqrt_price: Some(0),
        collect_fee_mode: Some(0),
        permanent_lock_liquidity: Some(0),
        pool_activation_type: Some(0),
    })
}

fn expected_core(token_a: Pubkey, token_b: Pubkey) -> PoolRegistryProperties {
    PoolRegistryProperties {
        token_a_mint: token_a,
        token_b_mint: token_b,
        fee_bps: Decimal::new(25, 0),
    }
}

/// Every write both fakes receive, in order, so a test asserts on the sequence.
type Journal = Arc<Mutex<Vec<Write>>>;

#[derive(Debug, PartialEq, Eq)]
enum Write {
    Satellite(Pubkey, PoolAccountProperties),
    Registry(Pubkey, PoolRegistryProperties),
}

#[derive(Default)]
struct FakeRepo {
    unresolved: Vec<Pubkey>,
    journal: Journal,
    /// When true, the satellite write fails, which must stop the registry write.
    fail: bool,
    /// What a successful satellite write reports as dropped.
    drops: Option<FieldOutOfRange>,
}

#[async_trait]
impl PoolAccountResolver for FakeRepo {
    fn protocol(&self) -> Protocol {
        Protocol::MeteoraDammV2
    }
    async fn list_unresolved(&self, _limit: i64) -> RepositoryResult<Vec<Pubkey>> {
        Ok(self.unresolved.clone())
    }
    async fn set_pool_account(
        &self,
        pool: &Pubkey,
        properties: &PoolAccountProperties,
    ) -> RepositoryResult<Option<FieldOutOfRange>> {
        if self.fail {
            return Err(yog_core::RepositoryError::Integrity("boom".into()));
        }
        self.journal
            .lock()
            .unwrap()
            .push(Write::Satellite(*pool, properties.clone()));
        Ok(self.drops)
    }
}

/// The neutral registry, sharing the journal with the resolver so the order is observable.
#[derive(Default)]
struct FakePoolRepo {
    journal: Journal,
}

#[async_trait]
impl PoolRepository for FakePoolRepo {
    async fn upsert(&self, _: &yog_core::domain::Pool) -> RepositoryResult<()> {
        unreachable!("the worker never discovers pools")
    }
    async fn touch_last_seen(&self, _: &Pubkey) -> RepositoryResult<()> {
        unreachable!("the worker never touches last_seen")
    }
    async fn mark_needs_refresh(&self, _: &Pubkey) -> RepositoryResult<()> {
        unreachable!("raising the flag is the indexer's job, never this worker's")
    }
    async fn set_registry_properties(
        &self,
        pool: &Pubkey,
        registry: &PoolRegistryProperties,
    ) -> RepositoryResult<()> {
        self.journal
            .lock()
            .unwrap()
            .push(Write::Registry(*pool, registry.clone()));
        Ok(())
    }
}

struct FakeSource {
    accounts: Vec<RawAccount>,
}

#[async_trait]
impl PoolAccountSource for FakeSource {
    async fn fetch_accounts(
        &self,
        _pool_addresses: &[Pubkey],
    ) -> Result<Vec<RawAccount>, SourceError> {
        Ok(self.accounts.clone())
    }
}

fn worker(
    repo: Arc<FakeRepo>,
    pool_repo: Arc<FakePoolRepo>,
    source: Arc<FakeSource>,
) -> PoolAccountWorker {
    PoolAccountWorker::new(
        vec![repo],
        pool_repo,
        source,
        std::time::Duration::from_secs(10),
        Arc::new(SourceHealth::default()),
    )
}

/// A resolver and a registry repository sharing one journal.
fn fakes(unresolved: Vec<Pubkey>, fail: bool) -> (Arc<FakeRepo>, Arc<FakePoolRepo>, Journal) {
    let journal: Journal = Arc::new(Mutex::new(Vec::new()));
    (
        Arc::new(FakeRepo {
            unresolved,
            journal: journal.clone(),
            fail,
            drops: None,
        }),
        Arc::new(FakePoolRepo {
            journal: journal.clone(),
        }),
        journal,
    )
}

/// Satellite first, registry last: the registry write lowers `needs_refresh`, so a satellite
/// failure must not leave a pool marked fresh with stale properties.
#[tokio::test]
async fn writes_both_halves_satellite_first() {
    let (repo, pool_repo, journal) = fakes(vec![pk(1)], false);
    let source = Arc::new(FakeSource {
        accounts: vec![RawAccount {
            pool_address: pk(1),
            program_id: Protocol::MeteoraDammV2.program_id(),
            data: cp_amm_account(pk(2), pk(3)),
        }],
    });

    worker(repo, pool_repo, source).run_one_cycle().await;

    assert_eq!(
        *journal.lock().unwrap(),
        vec![
            Write::Satellite(pk(1), expected_properties()),
            Write::Registry(pk(1), expected_core(pk(2), pk(3))),
        ]
    );
}

/// A failed satellite write is not followed by the registry write, or the pool would leave the
/// queue with stale satellite properties.
#[tokio::test]
async fn a_failed_satellite_write_does_not_clear_the_flag() {
    let (repo, pool_repo, journal) = fakes(vec![pk(1)], true);
    let source = Arc::new(FakeSource {
        accounts: vec![RawAccount {
            pool_address: pk(1),
            program_id: Protocol::MeteoraDammV2.program_id(),
            data: cp_amm_account(pk(2), pk(3)),
        }],
    });

    worker(repo, pool_repo, source).run_one_cycle().await;

    assert!(
        journal.lock().unwrap().is_empty(),
        "the registry write must be skipped when the satellite write failed"
    );
}

#[tokio::test]
async fn no_unresolved_pools_writes_nothing() {
    let (repo, pool_repo, journal) = fakes(Vec::new(), false);
    let source = Arc::new(FakeSource {
        accounts: Vec::new(),
    });

    worker(repo, pool_repo, source).run_one_cycle().await;

    assert!(journal.lock().unwrap().is_empty());
}

/// An account the decoder does not recognize is skipped, neither written nor fatal; it stays queued.
#[tokio::test]
async fn an_undecodable_account_is_skipped() {
    let (repo, pool_repo, journal) = fakes(vec![pk(1)], false);
    let source = Arc::new(FakeSource {
        accounts: vec![RawAccount {
            pool_address: pk(1),
            program_id: pk(99), // a program we do not index
            data: cp_amm_account(pk(2), pk(3)),
        }],
    });

    worker(repo, pool_repo, source).run_one_cycle().await;

    assert!(journal.lock().unwrap().is_empty());
}

/// An account of another protocol is never handed to this resolver.
#[tokio::test]
async fn an_account_of_another_protocol_is_not_written() {
    let (repo, pool_repo, journal) = fakes(vec![pk(1)], false);
    let source = Arc::new(FakeSource {
        accounts: vec![RawAccount {
            pool_address: pk(1),
            program_id: Protocol::MeteoraDlmm.program_id(),
            data: lb_pair_account(),
        }],
    });

    worker(repo, pool_repo, source).run_one_cycle().await;

    assert!(journal.lock().unwrap().is_empty());
}

// ── Heartbeat ─────────────────────────────────────────────────────────

use crate::application::heartbeat::window_verdict::WindowVerdict;

/// A source that refuses every call.
struct RefusingSource;

#[async_trait]
impl PoolAccountSource for RefusingSource {
    async fn fetch_accounts(&self, _: &[Pubkey]) -> Result<Vec<RawAccount>, SourceError> {
        Err(SourceError::RateLimited { retry_after: None })
    }
}

#[tokio::test]
async fn each_source_call_is_counted_for_the_heartbeat() {
    // Mutation: the call's outcome recorded backwards, or not at all.
    let (repo, pool_repo, _) = fakes(vec![pk(1)], false);
    let health = Arc::new(SourceHealth::default());
    let with_source = |source: Arc<dyn PoolAccountSource>| {
        PoolAccountWorker::new(
            vec![repo.clone()],
            pool_repo.clone(),
            source,
            std::time::Duration::from_secs(10),
            Arc::clone(&health),
        )
    };
    let refusing = with_source(Arc::new(RefusingSource));
    let answering = with_source(Arc::new(FakeSource { accounts: vec![] }));

    refusing.run_one_cycle().await;
    assert_eq!(
        health.take_window(),
        WindowVerdict::Failing(
            "pool_account: 1 calls, none succeeded, last error: source rate-limited (429), retry_after=None"
                .to_string()
        )
    );

    refusing.run_one_cycle().await;
    answering.run_one_cycle().await;
    assert_eq!(health.take_window(), WindowVerdict::Healthy);
}

// ── Metrics ───────────────────────────────────────────────────────────
//
// Not `#[tokio::test]`: `with_local_recorder` binds the current thread.

use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshotter};

type Snapshot = Vec<(
    metrics_util::CompositeKey,
    Option<::metrics::Unit>,
    Option<::metrics::SharedString>,
    DebugValue,
)>;

/// Drive one cycle under a thread-local recorder and return its snapshot.
fn snapshot_one_cycle(worker: PoolAccountWorker) -> Snapshot {
    let recorder = DebuggingRecorder::new();
    let snapshotter: Snapshotter = recorder.snapshotter();
    ::metrics::with_local_recorder(&recorder, || {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("current-thread runtime")
            .block_on(worker.run_one_cycle());
    });
    snapshotter.snapshot().into_vec()
}

/// The count under `name{protocol="meteora_damm_v2", reason}`, if any was recorded.
fn count(snapshot: &Snapshot, name: &str, reason: &str) -> Option<u64> {
    snapshot.iter().find_map(|(key, _, _, v)| {
        let labels: Vec<_> = key.key().labels().collect();
        let matches = key.key().name() == name
            && labels
                .iter()
                .any(|l| l.key() == "protocol" && l.value() == "meteora_damm_v2")
            && labels
                .iter()
                .any(|l| l.key() == "reason" && l.value() == reason);
        match (matches, v) {
            (true, DebugValue::Counter(n)) => Some(*n),
            _ => None,
        }
    })
}

fn one_account(data: Vec<u8>, program_id: Pubkey) -> Arc<FakeSource> {
    Arc::new(FakeSource {
        accounts: vec![RawAccount {
            pool_address: pk(1),
            program_id,
            data,
        }],
    })
}

/// Mutation: the rejection logged but not counted.
#[test]
fn a_rejected_account_is_counted_under_its_reason() {
    let (repo, pool_repo, _) = fakes(vec![pk(1)], false);
    let source = one_account(cp_amm_account(pk(2), pk(3)), pk(99));

    let snapshot = snapshot_one_cycle(worker(repo, pool_repo, source));

    assert_eq!(
        count(
            &snapshot,
            "yog_context_pool_account_rejected_total",
            "unknown_program"
        ),
        Some(1)
    );
}

/// Mutation: the decoder's degradations left uncounted, or the account no longer written.
#[test]
fn a_degraded_account_is_written_and_counted() {
    let (repo, pool_repo, journal) = fakes(vec![pk(1)], false);
    let mut data = cp_amm_account(pk(2), pk(3));
    data[16] = 99; // a BaseFeeMode no build maps

    let snapshot = snapshot_one_cycle(worker(
        repo,
        pool_repo,
        one_account(data, Protocol::MeteoraDammV2.program_id()),
    ));

    assert_eq!(
        count(
            &snapshot,
            "yog_context_pool_account_degraded_total",
            "base_fee_mode_unknown"
        ),
        Some(1)
    );
    assert_eq!(
        journal.lock().unwrap().len(),
        2,
        "both halves still written"
    );
}

/// Mutation: the repository's `Some` ignored, so the lost curve stays invisible.
#[test]
fn a_curve_the_store_cannot_hold_is_counted() {
    let journal: Journal = Arc::new(Mutex::new(Vec::new()));
    let repo = Arc::new(FakeRepo {
        unresolved: vec![pk(1)],
        journal: journal.clone(),
        fail: false,
        drops: Some(FieldOutOfRange {
            field: "period_frequency",
        }),
    });
    let pool_repo = Arc::new(FakePoolRepo {
        journal: journal.clone(),
    });
    let source = one_account(
        cp_amm_account(pk(2), pk(3)),
        Protocol::MeteoraDammV2.program_id(),
    );

    let snapshot = snapshot_one_cycle(worker(repo, pool_repo, source));

    assert_eq!(
        count(
            &snapshot,
            "yog_context_pool_account_degraded_total",
            "fee_scheduler_out_of_range"
        ),
        Some(1)
    );
    assert_eq!(
        journal.lock().unwrap().len(),
        2,
        "the registry is still written"
    );
}

/// Mutation: a whole account counted as degraded.
#[test]
fn a_whole_account_counts_nothing() {
    let (repo, pool_repo, _) = fakes(vec![pk(1)], false);
    let source = one_account(
        cp_amm_account(pk(2), pk(3)),
        Protocol::MeteoraDammV2.program_id(),
    );

    let snapshot = snapshot_one_cycle(worker(repo, pool_repo, source));

    assert!(
        snapshot
            .iter()
            .all(|(key, _, _, _)| !key.key().name().starts_with("yog_context_pool_account_")),
        "nothing to count: {snapshot:?}"
    );
}

/// Mutation: degradations counted before the satellite write, so a failing one counts every tick.
#[test]
fn a_failed_write_counts_no_degradation() {
    let (repo, pool_repo, _) = fakes(vec![pk(1)], true);
    let mut data = cp_amm_account(pk(2), pk(3));
    data[16] = 99; // a BaseFeeMode no build maps
    let source = one_account(data, Protocol::MeteoraDammV2.program_id());

    let snapshot = snapshot_one_cycle(worker(repo, pool_repo, source));

    assert_eq!(
        count(
            &snapshot,
            "yog_context_pool_account_degraded_total",
            "base_fee_mode_unknown"
        ),
        None,
        "nothing was written, so nothing was written without a field"
    );
}

/// Mutation: the skip left uncounted, so a pool requeued forever moves no series.
#[test]
fn an_account_of_another_protocol_is_counted_as_a_mismatch() {
    let (repo, pool_repo, _) = fakes(vec![pk(1)], false);
    let source = one_account(lb_pair_account(), Protocol::MeteoraDlmm.program_id());

    let snapshot = snapshot_one_cycle(worker(repo, pool_repo, source));

    assert_eq!(
        count(
            &snapshot,
            "yog_context_pool_account_rejected_total",
            "protocol_mismatch"
        ),
        Some(1)
    );
}
