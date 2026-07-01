//! Integration test for `PgSignalRepository::latest_severity_by_pool` — the
//! read backing the engine's cooldown / escalation dedup. Validates the
//! DISTINCT-ON latest-per-pool pick, the `since` window filter, and the
//! per-detector scoping.

#![cfg(feature = "integration-tests")]

use chrono::{Duration, Utc};
use solana_pubkey::Pubkey;
use sqlx::PgPool;

use yog_core::domain::{Severity, SignalRepository};
use yog_persistence::PgSignalRepository;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

async fn insert_signal(
    pool: &PgPool,
    detector: &str,
    pool_addr: &str,
    severity: &str,
    triggered_at: chrono::DateTime<Utc>,
) {
    sqlx::query(
        "INSERT INTO signals
           (detector, protocol, pool_address, severity, value, triggered_at)
         VALUES ($1, 'meteora_damm_v2', $2, $3, 1, $4)",
    )
    .bind(detector)
    .bind(pool_addr)
    .bind(severity)
    .bind(triggered_at)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test]
async fn latest_severity_by_pool_picks_latest_within_window_per_detector(pool: PgPool) {
    let now = Utc::now();
    let p1 = pk(1).to_string();
    let p2 = pk(2).to_string();
    let p3 = pk(3).to_string();

    // Pool 1: Warning then Critical → latest is Critical.
    insert_signal(
        &pool,
        "flow_imbalance",
        &p1,
        "warning",
        now - Duration::hours(3),
    )
    .await;
    insert_signal(
        &pool,
        "flow_imbalance",
        &p1,
        "critical",
        now - Duration::hours(1),
    )
    .await;
    // Pool 2: a single Warning inside the window.
    insert_signal(
        &pool,
        "flow_imbalance",
        &p2,
        "warning",
        now - Duration::hours(2),
    )
    .await;
    // Pool 3: only an old signal, outside the 24h window → must be absent.
    insert_signal(
        &pool,
        "flow_imbalance",
        &p3,
        "critical",
        now - Duration::hours(30),
    )
    .await;
    // Another detector on pool 2 → must not leak into this detector's map.
    insert_signal(
        &pool,
        "price_oracle_deviation",
        &p2,
        "critical",
        now - Duration::hours(1),
    )
    .await;

    let repo = PgSignalRepository::new(pool.clone());
    let map = repo
        .latest_severity_by_pool("flow_imbalance", now - Duration::hours(24))
        .await
        .unwrap();

    assert_eq!(map.get(&pk(1)), Some(&Severity::Critical), "latest wins");
    assert_eq!(map.get(&pk(2)), Some(&Severity::Warning));
    assert!(!map.contains_key(&pk(3)), "outside the window is excluded");
    assert_eq!(map.len(), 2, "other detectors are not counted");
}
