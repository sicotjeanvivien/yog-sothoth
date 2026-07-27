//! Integration tests for the ring-2 DAMM v2 event repositories.
//!
//! Gated behind the `integration-tests` feature: each test gets an isolated
//! Postgres database (via `sqlx::test`) with the migrations applied. The CI
//! job `test-integration` runs them; a plain `cargo test` skips them.
//!
//! These repos are write-only (no read method to assert against), so they
//! have no `rows_tests.rs` unit coverage and the SQL is only checked at
//! compile time by `sqlx::query!`. These tests close the runtime gap: that an
//! `insert` actually persists, that the type conversions survive a round trip
//! (u128 → NUMERIC(39,0) with no precision loss, fee blobs → BYTEA, u8 →
//! SMALLINT), and that the `ON CONFLICT (signature, timestamp) DO NOTHING`
//! idempotency guard holds.

#![cfg(feature = "integration-tests")]

// One file per event: Cargo treats `tests/<dir>/main.rs` as a SINGLE test
// target, so this stays one binary instead of the fifteen a flat split would
// have produced (measured: 13 binaries already cost ~6 s of relink on a lib
// touch).
mod helpers;

mod claim_protocol_fee;
mod close_position;
mod create_position;
mod fund_reward;
mod initialize_pool;
mod initialize_reward;
mod lock_position;
mod permanent_lock_position;
mod set_pool_status;
mod split_position;
mod update_pool_fees;
mod update_reward_duration;
mod update_reward_funder;
mod withdraw_dead_liquidity_reward;
mod withdraw_ineligible_reward;
