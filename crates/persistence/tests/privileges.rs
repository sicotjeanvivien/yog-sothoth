//! The privilege matrix: every grant the owner made to anyone else, compared both ways with
//! what the migrations produce. ⚠️ No role list: a role absent from the matrix is granted
//! nothing. Not covered: default privileges (`setup_roles.sql` never runs here), sequences,
//! functions. When it fails, read the diff as a question — *is this what I meant?* — before
//! updating the matrix.

use sqlx::PgPool;
use std::collections::BTreeSet;

/// What one role may do to one table: `(role, privileges)`.
type RolePrivileges<'a> = (&'a str, &'a [&'a str]);

/// One table's whole privilege surface: `(table, every role that may touch it)`.
type TablePrivileges<'a> = (&'a str, &'a [RolePrivileges<'a>]);

/// Table-level privileges, by table then role. `yog_indexer` holds UPDATE on every event table
/// though it updates none: the grant states ownership, not today's statements — do not tighten it.
const TABLE_PRIVILEGES: &[TablePrivileges] = &[
    (
        "analytic_hours",
        &[
            ("yog_analytic", &["INSERT", "SELECT", "UPDATE"]),
            ("yog_api", &["SELECT"]),
        ],
    ),
    (
        "analytic_pool_hourly",
        &[("yog_analytic", &["INSERT", "SELECT", "UPDATE"])],
    ),
    (
        "analytic_protocol_hourly",
        &[
            ("yog_analytic", &["INSERT", "SELECT", "UPDATE"]),
            ("yog_api", &["SELECT"]),
        ],
    ),
    (
        "analytic_protocol_tvl",
        &[
            ("yog_analytic", &["INSERT", "SELECT", "UPDATE"]),
            ("yog_api", &["SELECT"]),
        ],
    ),
    ("announcements", &[("yog_api", &["SELECT"])]),
    ("claim_position_fee_events", &[("yog_api", &["SELECT"])]),
    ("claim_protocol_fee_events", &[("yog_api", &["SELECT"])]),
    ("claim_reward_events", &[("yog_api", &["SELECT"])]),
    ("fund_reward_events", &[("yog_api", &["SELECT"])]),
    ("initialize_reward_events", &[("yog_api", &["SELECT"])]),
    // Default privileges cover these reads in production: only this matrix sees them go missing.
    ("liquidity_events", &[("yog_api", &["SELECT"])]),
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
        &[("yog_analytic", &["SELECT"]), ("yog_api", &["SELECT"])],
    ),
    (
        "meteora_damm_v2_swap_events_hourly_priced",
        &[("yog_analytic", &["SELECT"])],
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
    // yog-context owns it, yog_api reads it, the indexer has no business here.
    (
        "meteora_dlmm_pool_properties",
        &[
            ("yog_api", &["SELECT"]),
            ("yog_context", &["INSERT", "SELECT", "UPDATE"]),
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
        &[
            ("yog_analytic", &["SELECT"]),
            ("yog_api", &["SELECT"]),
            ("yog_signals", &["SELECT"]),
        ],
    ),
    ("pool_price_snapshot", &[("yog_signals", &["SELECT"])]),
    (
        "pools",
        &[
            ("yog_analytic", &["SELECT"]),
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
    ("swap_events", &[("yog_api", &["SELECT"])]),
    (
        "token_last_price",
        &[("yog_context", &["INSERT", "SELECT", "UPDATE"])],
    ),
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

/// Column-level privileges, which `role_table_grants` does not report:
/// `(table, role, privilege, columns)`.
const COLUMN_PRIVILEGES: &[(&str, &str, &str, &[&str])] = &[(
    "pools",
    "yog_context",
    "UPDATE",
    // Not table-level: `protocol`, `first_seen_at` and `last_seen_at` are the indexer's.
    &["fee_bps", "needs_refresh", "token_a_mint", "token_b_mint"],
)];

/// One `(table, role, privilege)` triple — the unit the table test compares on.
type Grant = (String, String, String);

/// One `(table, role, privilege, column)` — the unit the column test compares on.
type ColumnGrant = (String, String, String, String);

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

fn declared_column_grants() -> BTreeSet<ColumnGrant> {
    COLUMN_PRIVILEGES
        .iter()
        .flat_map(|(table, role, privilege, columns)| {
            columns.iter().map(move |column| {
                (
                    table.to_string(),
                    role.to_string(),
                    privilege.to_string(),
                    column.to_string(),
                )
            })
        })
        .collect()
}

/// Fails unless the two sets are equal, printing each difference as the SQL that closes it.
/// Exact equality: an undeclared privilege widens what a buggy process can reach.
fn assert_same_surface<T: Ord + Clone>(
    matrix: &str,
    hint: &str,
    declared: &BTreeSet<T>,
    observed: &BTreeSet<T>,
    render: impl Fn(&T, &str, &str) -> String,
) {
    let missing: Vec<String> = declared
        .difference(observed)
        .map(|grant| render(grant, "GRANT", "TO"))
        .collect();
    let excess: Vec<String> = observed
        .difference(declared)
        .map(|grant| render(grant, "REVOKE", "FROM"))
        .collect();

    assert!(
        missing.is_empty() && excess.is_empty(),
        "`{matrix}` and the migrated schema disagree.\n\n\
         Declared but NOT granted ({} — a migration is missing its GRANT, and the \
         process that needs it will fail at runtime under its real role):\n{}\n\n\
         Granted but NOT declared ({} — either a migration granted more than \
         intended, or the matrix is out of date):\n{}\n\n\
         {hint}Update `{matrix}` in this file only after deciding which of the two is wrong.",
        missing.len(),
        missing.join("\n"),
        excess.len(),
        excess.join("\n"),
    );
}

/// The whole table-level surface the owner granted, in both directions.
#[sqlx::test]
async fn table_privileges_match_the_declared_matrix(pool: PgPool) {
    let observed: BTreeSet<Grant> = sqlx::query_as(
        r#"
        SELECT table_name::TEXT, grantee::TEXT, privilege_type::TEXT
        FROM information_schema.role_table_grants
        WHERE table_schema = 'public'
          AND grantee <> current_user
        "#,
    )
    .fetch_all(&pool)
    .await
    .expect("privilege query failed")
    .into_iter()
    .collect();

    assert_same_surface(
        "TABLE_PRIVILEGES",
        "",
        &declared_table_grants(),
        &observed,
        |(table, role, privilege), statement, direction| {
            format!("    {statement} {privilege} ON {table} {direction} {role};")
        },
    );
}

/// Column-level grants, which do not extend to a column added later. ⚠️ From `pg_attribute.attacl`,
/// not `column_privileges`, which repeats every table-level grant on each column.
#[sqlx::test]
async fn column_privileges_match_the_declared_matrix(pool: PgPool) {
    let observed: BTreeSet<ColumnGrant> = sqlx::query_as(
        r#"
        SELECT c.relname::TEXT,
               COALESCE(r.rolname::TEXT, 'PUBLIC'),
               acl.privilege_type::TEXT,
               a.attname::TEXT
        FROM pg_attribute a
        JOIN pg_class c ON c.oid = a.attrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        CROSS JOIN LATERAL aclexplode(a.attacl) acl
        LEFT JOIN pg_roles r ON r.oid = acl.grantee
        WHERE n.nspname = 'public'
          AND c.relkind IN ('r', 'v', 'f', 'p')
          AND a.attnum > 0
          AND NOT a.attisdropped
          AND acl.grantee <> c.relowner
        "#,
    )
    .fetch_all(&pool)
    .await
    .expect("column privilege query failed")
    .into_iter()
    .collect();

    assert_same_surface(
        "COLUMN_PRIVILEGES",
        "A column added later is NOT covered by an existing column grant: the \
         migration that adds it owes it an explicit one.\n\n",
        &declared_column_grants(),
        &observed,
        |(table, role, privilege, column), statement, direction| {
            format!("    {statement} {privilege} ({column}) ON {table} {direction} {role};")
        },
    );
}
