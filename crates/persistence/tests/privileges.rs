//! The privilege matrix: what each runtime role may do, asserted against the
//! database the migrations actually produce.
//!
//! # Why this exists
//!
//! `CLAUDE.md` presents the role split as a safety net — "calling `insert` from
//! the api process fails with `permission denied` *by design*". Nothing verified
//! it. Migration 036 moved the fee-shape columns off `pools` (where `yog_indexer`
//! held table-level UPDATE) onto a satellite it granted only to `yog_context`
//! and `yog_api`. Every `set_fee_config` from the indexer failed with
//! `permission denied` for a month, invisibly: those writes are skip-and-log,
//! no `InitializePool` event was observed in that window, and the tests run as
//! the owner. A safety net nothing checks is not a net.
//!
//! The failure is not "a GRANT was forgotten once". It is that **every migration
//! must remember its GRANTs by hand and nothing compares the result to an
//! intent**. This module is that intent, written by hand.
//!
//! # Scope, and why it stops there
//!
//! **Explicit grants only** — those a migration emits. Measured: they reproduce
//! faithfully in a `sqlx::test` database, column-level ones included, and that is
//! precisely where migration 036's bug lived.
//!
//! The **default privileges** of `setup_roles.sql` (a blanket `SELECT` to every
//! role) do *not* reproduce, for two compounding reasons: that file is not a
//! migration, so it never runs here; and `ALTER DEFAULT PRIVILEGES FOR ROLE
//! yog_migrate` only applies to objects *created by* `yog_migrate`, whereas
//! `sqlx::test` applies migrations as the connecting user. Running the
//! migrations as `yog_migrate` is not an option either — that role has no
//! `CREATEDB`, and granting it one would weaken the model to serve a test.
//!
//! So a role missing its blanket `SELECT` in production is **not** caught here.
//! Neither is code that exceeds the rights the matrix grants it — that would
//! need a test writing under the real role, deliberately left for later.
//!
//! `yog_migrate` is absent from the matrix on purpose: its rights come from
//! owning the schema, not from grants, and the owner differs between this
//! database (the connecting user) and production (`yog_migrate`).
//!
//! # When this test fails
//!
//! It is telling you a migration changed the privilege surface. Read the diff it
//! prints as a question — *is this what I meant?* — and only then update the
//! matrix. Pasting the missing line to make it green turns the guard back into
//! the formality it was written to replace.

use sqlx::PgPool;
use std::collections::BTreeSet;

/// The four runtime roles. `yog_migrate` is excluded — see the module doc.
const RUNTIME_ROLES: [&str; 4] = ["yog_api", "yog_context", "yog_indexer", "yog_signals"];

/// What one role may do to one table: `(role, privileges)`.
type RolePrivileges<'a> = (&'a str, &'a [&'a str]);

/// One table's whole privilege surface: `(table, every role that may touch it)`.
type TablePrivileges<'a> = (&'a str, &'a [RolePrivileges<'a>]);

/// Table-level privileges, by table then role.
///
/// ## `yog_indexer` owns the event tables outright
///
/// It holds `SELECT, INSERT, UPDATE` on all 22 DAMM v2 event tables even though
/// it currently updates none of them — every insert is `ON CONFLICT … DO
/// NOTHING`. **That is intended, and the uniform grant is the point**: the
/// indexer owns its event tables, and the grant states ownership rather than
/// today's statements. `crates/README.md` says the same — "RW on event tables".
///
/// Recorded here so nobody re-derives an intent from usage and "tightens" it: an
/// event table that later needs a corrective UPDATE must not require a migration
/// to get a right it was always meant to have.
const TABLE_PRIVILEGES: &[TablePrivileges] = &[
    ("announcements", &[("yog_api", &["SELECT"])]),
    ("claim_position_fee_events", &[("yog_api", &["SELECT"])]),
    ("claim_protocol_fee_events", &[("yog_api", &["SELECT"])]),
    ("claim_reward_events", &[("yog_api", &["SELECT"])]),
    ("fund_reward_events", &[("yog_api", &["SELECT"])]),
    ("initialize_reward_events", &[("yog_api", &["SELECT"])]),
    (
        "meteora_damm_v2_claim_position_fee_events",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_indexer", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "meteora_damm_v2_claim_position_fee_events_hourly",
        &[("yog_api", &["SELECT"])],
    ),
    (
        "meteora_damm_v2_claim_protocol_fee_events",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_indexer", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "meteora_damm_v2_claim_reward_events",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_indexer", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "meteora_damm_v2_claim_reward_events_hourly",
        &[("yog_api", &["SELECT"])],
    ),
    (
        "meteora_damm_v2_close_position_events",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_indexer", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "meteora_damm_v2_create_position_events",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_indexer", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "meteora_damm_v2_fund_reward_events",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_indexer", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "meteora_damm_v2_initialize_pool_events",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_indexer", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "meteora_damm_v2_initialize_reward_events",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_indexer", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "meteora_damm_v2_liquidity_events",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_indexer", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "meteora_damm_v2_liquidity_events_hourly",
        &[("yog_api", &["SELECT"])],
    ),
    (
        "meteora_damm_v2_liquidity_events_valued",
        &[("yog_api", &["SELECT"])],
    ),
    (
        "meteora_damm_v2_lock_position_events",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_indexer", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "meteora_damm_v2_permanent_lock_position_events",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_indexer", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "meteora_damm_v2_pool_hourly_activity",
        &[("yog_api", &["SELECT"])],
    ),
    (
        "meteora_damm_v2_pool_hourly_flow",
        &[("yog_signals", &["SELECT"])],
    ),
    (
        "meteora_damm_v2_pool_hourly_liquidity_flow",
        &[("yog_signals", &["SELECT"])],
    ),
    (
        "meteora_damm_v2_pool_properties",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_context", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "meteora_damm_v2_set_pool_status_events",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_indexer", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "meteora_damm_v2_split_position_events",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_indexer", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "meteora_damm_v2_swap_events",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_indexer", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "meteora_damm_v2_swap_events_hourly",
        &[("yog_api", &["SELECT"])],
    ),
    (
        "meteora_damm_v2_update_pool_fees_events",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_indexer", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "meteora_damm_v2_update_reward_duration_events",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_indexer", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "meteora_damm_v2_update_reward_funder_events",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_indexer", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "meteora_damm_v2_withdraw_dead_liquidity_reward_events",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_indexer", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "meteora_damm_v2_withdraw_ineligible_reward_events",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_indexer", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "network_status",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_indexer", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "pool_current_state",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_indexer", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "pool_current_tvl",
        &[("yog_api", &["SELECT"]), ("yog_signals", &["SELECT"])],
    ),
    ("pool_price_snapshot", &[("yog_signals", &["SELECT"])]),
    (
        "pools",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_context", &["SELECT"]),
            ("yog_indexer", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "signals",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_signals", &["INSERT", "SELECT"]),
        ],
    ),
    ("split_position_events", &[("yog_api", &["SELECT"])]),
    (
        "token_metadata",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_context", &["INSERT", "SELECT", "UPDATE"]),
        ],
    ),
    (
        "token_prices",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_context", &["INSERT", "SELECT"]),
        ],
    ),
    ("update_reward_duration_events", &[("yog_api", &["SELECT"])]),
    ("update_reward_funder_events", &[("yog_api", &["SELECT"])]),
    (
        "watched_pools",
        &[("yog_api", &["SELECT"]), ("yog_indexer", &["SELECT"])],
    ),
    (
        "withdraw_dead_liquidity_reward_events",
        &[("yog_api", &["SELECT"])],
    ),
    (
        "withdraw_ineligible_reward_events",
        &[("yog_api", &["SELECT"])],
    ),
];

/// Column-level privileges, kept apart because `role_table_grants` does not
/// report them — a column grant is invisible to the table-level query, which is
/// exactly how one would slip through unnoticed.
///
/// `(table, role, privilege, columns)`.
const COLUMN_PRIVILEGES: &[(&str, &str, &str, &[&str])] = &[(
    "pools",
    "yog_context",
    "UPDATE",
    // The account-derived columns yog-context owns. Deliberately *not*
    // table-level: `protocol`, `first_seen_at` and `last_seen_at` are the
    // indexer's, and a column list is what keeps that boundary real.
    &["fee_bps", "needs_refresh", "token_a_mint", "token_b_mint"],
)];

/// One `(table, role, privilege)` triple — the unit both tests compare on.
type Grant = (String, String, String);

fn declared_table_grants() -> BTreeSet<Grant> {
    TABLE_PRIVILEGES
        .iter()
        .flat_map(|(table, roles)| {
            roles.iter().flat_map(move |(role, privileges)| {
                privileges
                    .iter()
                    .map(move |p| (table.to_string(), role.to_string(), p.to_string()))
            })
        })
        .collect()
}

/// Render a difference as the SQL that would close it, so the failure is
/// actionable without a second lookup.
fn render(grants: &BTreeSet<Grant>, statement: &str, direction: &str) -> String {
    grants
        .iter()
        .map(|(table, role, privilege)| {
            format!("    {statement} {privilege} ON {table} {direction} {role};")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The whole table-level surface, in both directions.
///
/// Exact equality, not "at least": a privilege nobody declared is as much a
/// finding as one that went missing — it silently widens what a compromised or
/// buggy process can reach.
#[sqlx::test]
async fn table_privileges_match_the_declared_matrix(pool: PgPool) {
    let rows: Vec<Grant> = sqlx::query_as(
        r#"
        SELECT table_name::TEXT, grantee::TEXT, privilege_type::TEXT
        FROM information_schema.role_table_grants
        WHERE table_schema = 'public'
          AND grantee = ANY($1)
        "#,
    )
    .bind(RUNTIME_ROLES.map(str::to_string).to_vec())
    .fetch_all(&pool)
    .await
    .expect("privilege query failed");

    let observed: BTreeSet<Grant> = rows.into_iter().collect();
    let declared = declared_table_grants();

    let missing: BTreeSet<Grant> = declared.difference(&observed).cloned().collect();
    let excess: BTreeSet<Grant> = observed.difference(&declared).cloned().collect();

    assert!(
        missing.is_empty() && excess.is_empty(),
        "the privilege matrix and the migrated schema disagree.\n\n\
         Declared but NOT granted ({} — a migration is missing its GRANT, and \
         the process that needs it will fail at runtime under its real role):\n{}\n\n\
         Granted but NOT declared ({} — either a migration granted more than \
         intended, or the matrix is out of date):\n{}\n\n\
         Update `TABLE_PRIVILEGES` in this file only after deciding which of the \
         two is wrong.",
        missing.len(),
        render(&missing, "GRANT", "TO"),
        excess.len(),
        render(&excess, "REVOKE", "FROM"),
    );
}

/// Column-level grants, which the query above cannot see.
///
/// A column grant is the finest tool in the model — `yog_context` may write four
/// columns of `pools` and no others — and also the easiest to lose: it does not
/// extend to a column added later, which is the trap migration 038 had to work
/// around explicitly.
#[sqlx::test]
async fn column_privileges_match_the_declared_matrix(pool: PgPool) {
    for (table, role, privilege, columns) in COLUMN_PRIVILEGES {
        let observed: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT column_name::TEXT
            FROM information_schema.column_privileges
            WHERE table_schema = 'public'
              AND table_name = $1 AND grantee = $2 AND privilege_type = $3
            ORDER BY column_name
            "#,
        )
        .bind(table)
        .bind(role)
        .bind(privilege)
        .fetch_all(&pool)
        .await
        .expect("column privilege query failed");

        let mut declared: Vec<String> = columns.iter().map(|c| c.to_string()).collect();
        declared.sort();

        assert_eq!(
            observed, declared,
            "{role} may {privilege} the wrong columns of {table}.\n\
             A column added later is NOT covered by an existing column grant — \
             if this is a new column, the migration owes it an explicit \
             `GRANT {privilege} (<column>) ON {table} TO {role};`."
        );
    }
}
