use super::pg_names::*;
use super::replay::replay;
use super::scan::{migrations_dir, scan_file, scan_migrations, scan_sql};
use super::*;

/// Fixtures read off a live `timescale/timescaledb:latest-pg16` before being
/// asserted here: the one collision the schema already carries, and the
/// truncated default index `create_hypertable` builds on the longest table.
const DURATION_TABLE: &str = "meteora_damm_v2_update_reward_duration_events";
const FUNDER_TABLE: &str = "meteora_damm_v2_update_reward_funder_events";
const DURATION_INDEX: &str = "meteora_damm_v2_update_reward_signature_event_index_timesta_idx";
const FUNDER_INDEX: &str = "meteora_damm_v2_update_reward_signature_event_index_timest_idx1";
const LONGEST_TABLE: &str = "meteora_damm_v2_withdraw_dead_liquidity_reward_events";
const LONGEST_HYPERTABLE_INDEX: &str =
    "meteora_damm_v2_withdraw_dead_liquidity_reward_ev_timestamp_idx";

const KEY_COLUMNS: [&str; 3] = ["signature", "event_index", "timestamp"];

fn key_columns() -> Vec<String> {
    KEY_COLUMNS.iter().map(|c| c.to_string()).collect()
}

fn decl(table: &str, columns: &[&str]) -> IndexDecl {
    IndexDecl {
        file: "synthetic.sql".to_string(),
        line: 1,
        explicit_name: None,
        table: table.to_string(),
        columns: columns.iter().map(|c| c.to_string()).collect(),
        unique: false,
        origin: Origin::Written,
    }
}

fn collided(decls: Vec<IndexDecl>) -> Vec<Collision> {
    replay(&decls.into_iter().map(Event::Index).collect::<Vec<_>>())
        .expect("these fixtures replay cleanly")
        .collisions
}

/// Scan then replay, the way the guard does over the real files.
fn run(sql: &str) -> Replay {
    let scanned = scan_sql("synthetic.sql", sql).expect("this DDL must parse");
    replay(&scanned.events).expect("this DDL must replay")
}

fn scan_err(sql: &str) -> String {
    scan_sql("synthetic.sql", sql).expect_err("this DDL must be refused")
}

fn replay_migrations() -> Replay {
    let scanned = scan_migrations().expect("every migration must be parseable");
    replay(&scanned.events).expect("every migration must replay")
}

mod guard_tests;
mod lexer_tests;
mod pg_names_tests;
mod replay_tests;
mod scan_tests;
