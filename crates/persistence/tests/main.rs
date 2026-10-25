//! Integration tests for `yog-persistence`, gated on `integration-tests` by the `#![cfg]`
//! below — not `#[ignore]`d. They need a live Postgres reached as admin (see CLAUDE.md).
//! ⚠️ One test binary: `autotests = false` makes this the only target, so a new
//! `tests/<subject>.rs` must be declared below.
#![cfg(feature = "integration-tests")]

mod helpers;

mod amount_usd;
mod analytic_bursts;
mod analytic_liquidity;
mod analytic_prices;
mod analytic_rebuild;
mod analytic_writer;
mod announcement_entries;
mod archive_role;
mod cagg_materialization;
mod cagg_retention;
mod claim_caggs;
mod claim_protocol_fee;
mod close_position;
mod create_position;
mod discovered_tvl;
mod event_index_uniqueness;
mod fee_split;
mod fund_reward;
mod implied_price_coverage;
mod initialize_pool;
mod initialize_reward;
mod liquidity_cagg;
mod liquidity_change_events;
mod liquidity_flow;
mod liquidity_moves;
mod liquidity_value;
mod lock_position;
mod overview_periods;
mod permanent_lock_position;
mod pool_change_events;
mod pool_creation_events;
mod pool_creations;
mod pool_current_state_order;
mod pool_entries;
mod pool_price_snapshot;
mod pool_properties;
mod pool_registry;
mod position_change_events;
mod position_fee_claim_events;
mod price_compression;
mod price_positivity;
mod price_staleness;
mod privileges;
mod protocol_activity;
mod protocol_fee_claim_events;
mod protocol_tvl;
mod reward_change_events;
mod reward_claim_events;
mod reward_valuation;
mod runtime_role_writes;
mod set_pool_status;
mod signal_dedup;
mod signal_list;
mod split_position;
mod statement_timeout;
mod swap_activity;
mod swap_events;
mod swap_flow;
mod swap_retention;
mod token_entries;
mod token_last_price;
mod token_metadata;
mod token_prices;
mod update_pool_fees;
mod update_reward_duration;
mod update_reward_funder;
mod volume_cagg;
mod withdraw_dead_liquidity_reward;
mod withdraw_ineligible_reward;
