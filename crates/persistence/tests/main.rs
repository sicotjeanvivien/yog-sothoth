//! Integration tests for `yog-persistence`, DB-backed and `#[ignore]`d by
//! default — they need a live Postgres (see CLAUDE.md for the
//! `timescaledb.max_background_workers = 0` requirement).
//!
//! **One file per subject, one single test binary.** Cargo auto-discovers every
//! `.rs` directly under `tests/` as its own target, which would mean one link
//! of the whole crate per file. `autotests = false` in Cargo.toml turns that
//! off and declares this file as the only target; everything else is a plain
//! module. Measured before choosing: 13 separate binaries cost ~6 s of relink
//! on a lib touch, and a file-per-event split would have taken that past 27.
//!
//! Add a test file: create `tests/<subject>.rs` and declare it below.
#![cfg(feature = "integration-tests")]

mod helpers;

mod claim_caggs;
mod claim_protocol_fee;
mod close_position;
mod create_position;
mod event_index_uniqueness;
mod fund_reward;
mod initialize_pool;
mod initialize_reward;
mod liquidity_cagg;
mod liquidity_flow;
mod liquidity_value;
mod lock_position;
mod permanent_lock_position;
mod pool_analytics_ranking;
mod pool_current_state_order;
mod pool_pagination;
mod pool_price_snapshot;
mod pool_properties;
mod pool_search;
mod privileges;
mod set_pool_status;
mod signal_dedup;
mod signal_list;
mod split_position;
mod swap_flow;
mod token_metadata;
mod update_pool_fees;
mod update_reward_duration;
mod update_reward_funder;
mod volume_cagg;
mod withdraw_dead_liquidity_reward;
mod withdraw_ineligible_reward;
