//! The `yog_archive` role: it reads everything `pg_dump` needs, and writes
//! nothing.
//!
//! Its rights do not come from a migration, so `privileges.rs` cannot see
//! them: `setup_roles.sql` makes it a member of the predefined
//! `pg_read_all_data`. These tests apply that file, then act **under the role
//! itself** (`SET ROLE`), because a privilege is only proven by the statement
//! it lets through or stops.
//!
//! ⚠️ **Roles belong to the cluster, not to the test's database.** A grant made
//! by an earlier run survives the removal of the `GRANT` from the file, and a
//! test that only ran the file would stay green on a script that no longer
//! grants anything. So the membership is revoked first: what the assertions
//! see is what the file grants *now*. Measured by mutation — with the `GRANT`
//! line removed, the read test fails on `42501`.

use super::helpers::sqlstate;
use sqlx::PgPool;
use yog_persistence::PgServerInfo;

const SETUP_ROLES_SQL: &str = include_str!("../src/bin/scripts/setup_roles.sql");

/// `42501` — insufficient_privilege.
const INSUFFICIENT_PRIVILEGE: &str = "42501";

/// Apply `setup_roles.sql` from a state where `yog_archive` holds nothing.
async fn setup_roles_from_scratch(pool: &PgPool) {
    sqlx::raw_sql(
        "DO $$ BEGIN
            IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'yog_archive') THEN
                REVOKE pg_read_all_data FROM yog_archive;
            END IF;
        END $$;",
    )
    .execute(pool)
    .await
    .expect("revoke the membership an earlier run left");

    sqlx::raw_sql(SETUP_ROLES_SQL)
        .execute(pool)
        .await
        .expect("apply setup_roles.sql");
}

/// One test, not two: the role is cluster-wide, and `sqlx::test` runs tests in
/// parallel, each in its own database. A second test revoking the membership
/// while this one reads fails it at random — observed on the first run — and
/// an advisory lock cannot serialise them, since those are per-database.
#[sqlx::test]
async fn yog_archive_reads_everything_and_writes_nothing(pool: PgPool) {
    setup_roles_from_scratch(&pool).await;
    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("SET ROLE yog_archive")
        .execute(&mut *conn)
        .await
        .unwrap();

    // `pools` grants nothing to yog_archive by name: only the membership can
    // let this through. Likewise TimescaleDB's catalog, which `pg_dump` reads
    // to dump a hypertable and its chunks.
    let pools: i64 = sqlx::query_scalar("SELECT count(*) FROM pools")
        .fetch_one(&mut *conn)
        .await
        .expect("yog_archive must read a public table");
    assert_eq!(pools, 0);

    let catalog: i64 = sqlx::query_scalar("SELECT count(*) FROM _timescaledb_catalog.hypertable")
        .fetch_one(&mut *conn)
        .await
        .expect("yog_archive must read TimescaleDB's catalog");
    assert!(catalog > 0, "the migrations create hypertables");

    // The privilege check runs before any constraint, so an otherwise invalid
    // row still fails on the privilege and nothing else.
    for statement in [
        "INSERT INTO pools DEFAULT VALUES",
        "UPDATE pools SET needs_refresh = true",
        "DELETE FROM token_prices",
        "CREATE TABLE archive_should_not_create (id int)",
    ] {
        let err = sqlx::query(statement)
            .execute(&mut *conn)
            .await
            .expect_err(statement);
        assert_eq!(sqlstate(&err), INSUFFICIENT_PRIVILEGE, "{statement}");
    }
}

#[sqlx::test]
async fn server_versions_names_the_postgres_major_and_timescaledb(pool: PgPool) {
    let versions = PgServerInfo::new(pool.clone())
        .server_versions()
        .await
        .unwrap();

    let expected_major: i32 =
        sqlx::query_scalar("SELECT current_setting('server_version_num')::int / 10000")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(versions.postgres_major as i32, expected_major);

    let parts: Vec<&str> = versions.timescaledb.split('.').collect();
    assert_eq!(parts.len(), 3, "{}", versions.timescaledb);
    assert!(
        parts.iter().all(|p| p.parse::<u32>().is_ok()),
        "{}",
        versions.timescaledb
    );
}
