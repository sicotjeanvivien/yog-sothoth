//! The writes a runtime role receives without any grant naming it, taken back.
//!
//! `PUBLIC` holds two rights by default that let a role the matrix calls
//! read-only write anyway: `EXECUTE` on TimescaleDB's job API, and `TEMP` on
//! the database. `setup_roles.sql` revokes both, so these tests apply it, then
//! act **under the role** (`SET ROLE`). A third write, `yog_indexer` drawing
//! from the sequence of a table it cannot write, is closed by migration 012.
//!
//! Each refusal is asserted on its SQLSTATE **and** on the object the message
//! names. `42501` alone would also be satisfied by a refusal on something else
//! in the statement, which would stay green once the REVOKE under test is gone.

use super::helpers::{INSUFFICIENT_PRIVILEGE, apply_setup_roles, sqlstate};
use sqlx::pool::PoolConnection;
use sqlx::postgres::Postgres;
use sqlx::{AssertSqlSafe, PgPool};

// `AssertSqlSafe` below: every statement is a literal of this file, and the
// role and sequence names interpolated into two of them are constants — a role
// name cannot be a bind parameter.

/// Every role a runtime process connects under.
const RUNTIME_ROLES: [&str; 5] = [
    "yog_indexer",
    "yog_api",
    "yog_context",
    "yog_signals",
    "yog_archive",
];

async fn connect_as(pool: &PgPool, role: &str) -> PoolConnection<Postgres> {
    let mut conn = pool.acquire().await.unwrap();
    sqlx::query(AssertSqlSafe(format!("SET ROLE {role}")))
        .execute(&mut *conn)
        .await
        .unwrap();
    conn
}

/// Run `statement` and require the insufficient-privilege refusal, with
/// `reason` in its message.
async fn assert_refused(
    conn: &mut PoolConnection<Postgres>,
    role: &str,
    statement: &str,
    reason: &str,
) {
    let err = sqlx::query(AssertSqlSafe(statement.to_owned()))
        .execute(&mut **conn)
        .await
        .expect_err(&format!("{role}: {statement}"));
    assert_eq!(
        sqlstate(&err),
        INSUFFICIENT_PRIVILEGE,
        "{role}: {statement}: {err}"
    );
    let message = err.as_database_error().unwrap().message().to_owned();
    assert!(
        message.contains(reason),
        "{role}: {statement}: refused for another reason: {message}"
    );
}

#[sqlx::test]
async fn no_runtime_role_can_schedule_or_touch_a_job(pool: PgPool) {
    apply_setup_roles(&pool).await;

    for role in RUNTIME_ROLES {
        let mut conn = connect_as(&pool, role).await;
        assert_refused(
            &mut conn,
            role,
            "SELECT add_job('pg_catalog.pg_sleep', INTERVAL '1 second')",
            "function add_job",
        )
        .await;
        // Why the message is asserted too: with `alter_job` left executable,
        // TimescaleDB's own check on the job answers the same `42501`
        // ("insufficient permissions to alter job 1") — measured by mutation.
        // Only the object the message names tells the two refusals apart.
        assert_refused(
            &mut conn,
            role,
            "SELECT alter_job(1, scheduled => false)",
            "function alter_job",
        )
        .await;
        assert_refused(
            &mut conn,
            role,
            "SELECT delete_job(1)",
            "function delete_job",
        )
        .await;
        assert_refused(&mut conn, role, "CALL run_job(1)", "procedure run_job").await;
    }
}

#[sqlx::test]
async fn no_runtime_role_can_create_a_temporary_table(pool: PgPool) {
    apply_setup_roles(&pool).await;

    for role in RUNTIME_ROLES {
        let mut conn = connect_as(&pool, role).await;
        assert_refused(
            &mut conn,
            role,
            "CREATE TEMP TABLE runtime_role_scratch (id int)",
            "temporary tables",
        )
        .await;
    }
}

#[sqlx::test]
async fn yog_indexer_draws_only_from_the_sequences_of_its_tables(pool: PgPool) {
    let mut conn = connect_as(&pool, "yog_indexer").await;

    // The witness: a sequence behind a table it writes still serves it. Without
    // it, the refusals below would also hold for a role that can draw nothing.
    sqlx::query("SELECT nextval('meteora_damm_v2_swap_events_id_seq')")
        .execute(&mut *conn)
        .await
        .expect("yog_indexer draws the ids of the events it inserts");

    for sequence in ["signals_id_seq", "announcements_id_seq"] {
        assert_refused(
            &mut conn,
            "yog_indexer",
            &format!("SELECT nextval('{sequence}')"),
            &format!("sequence {sequence}"),
        )
        .await;
    }
}
