//! The `yog_archive` role: it reads every schema `pg_dump` reads, what is born
//! after `setup_roles.sql` included, and writes nothing — nor reads `pg_authid`.
//! Those grants come from that file, not from a migration, so `privileges.rs`
//! cannot see them: these tests apply it, then act under the role.

use super::helpers::{
    CLUSTER_ROLES, INSUFFICIENT_PRIVILEGE, SETUP_ROLES_SQL, assert_reads_no_password_hash, sqlstate,
};
use sqlx::{AssertSqlSafe, PgPool};
use yog_persistence::PgServerInfo;

/// The file applied as production meets it: a schema the migrations did not
/// make, and the `pg_read_all_data` membership `yog_archive` used to hold.
async fn setup_roles_over_the_previous_state(pool: &PgPool) {
    let _cluster = CLUSTER_ROLES.lock().await;
    sqlx::raw_sql(
        "CREATE SCHEMA archive_probe;
         CREATE TABLE archive_probe.kept (id int);
         GRANT pg_read_all_data TO yog_archive;",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::raw_sql(SETUP_ROLES_SQL)
        .execute(pool)
        .await
        .expect("apply setup_roles.sql");
}

async fn scalar(pool: &PgPool, query: &'static str) -> String {
    sqlx::query_scalar(query).fetch_one(pool).await.unwrap()
}

/// What a migration and the jobs make after the file ran: under yog_migrate a
/// compressed hypertable and a continuous aggregate, then a chunk of
/// `token_prices` born late and one compressed. Returns the relations to read.
async fn born_after_the_file(pool: &PgPool) -> Vec<String> {
    let mut migrate = pool.acquire().await.unwrap();
    for statement in [
        "SET ROLE yog_migrate",
        "CREATE TABLE archive_late (ts TIMESTAMPTZ NOT NULL, v BIGINT NOT NULL)",
        "SELECT create_hypertable('archive_late', 'ts', create_default_indexes => FALSE)",
        "ALTER TABLE archive_late SET (timescaledb.compress)",
        "INSERT INTO archive_late VALUES (TIMESTAMPTZ '2020-01-01', 1)",
        "SELECT compress_chunk(c) FROM show_chunks('archive_late') c",
        "CREATE MATERIALIZED VIEW archive_late_hourly WITH (timescaledb.continuous) AS
           SELECT time_bucket('1 hour', ts) AS bucket, sum(v) AS v FROM archive_late
           GROUP BY 1 WITH NO DATA",
        "RESET ROLE",
        "INSERT INTO token_prices (mint, price_usd, price_provider, fetched_at)
         VALUES ('archive_probe', 1, 'jupiter', TIMESTAMPTZ '2030-01-01'),
                ('archive_probe', 1, 'jupiter', TIMESTAMPTZ '2020-01-01')",
        "SELECT compress_chunk(c)
           FROM show_chunks('token_prices', older_than => TIMESTAMPTZ '2020-02-01') c",
    ] {
        sqlx::query(statement)
            .execute(&mut *migrate)
            .await
            .unwrap_or_else(|e| panic!("{statement}: {e}"));
    }
    vec![
        "archive_late".to_string(),
        scalar(
            pool,
            "SELECT s.compress_relid::TEXT FROM _timescaledb_catalog.compression_settings s
               JOIN show_chunks('archive_late') c ON s.relid = c",
        )
        .await,
        scalar(
            pool,
            "SELECT format('%I.%I', materialization_hypertable_schema,
                                    materialization_hypertable_name)
               FROM timescaledb_information.continuous_aggregates
              WHERE view_name = 'archive_late_hourly'",
        )
        .await,
        scalar(
            pool,
            "SELECT c::TEXT FROM show_chunks('token_prices',
                                             newer_than => TIMESTAMPTZ '2029-12-01') c",
        )
        .await,
        scalar(
            pool,
            "SELECT s.compress_relid::TEXT FROM _timescaledb_catalog.compression_settings s
               JOIN show_chunks('token_prices', older_than => TIMESTAMPTZ '2020-02-01') c
                 ON s.relid = c",
        )
        .await,
    ]
}

/// Against the default privileges dropped, the grants of the file kept to
/// `public`, and the old membership left in place.
#[sqlx::test]
async fn yog_archive_reads_every_schema_and_writes_nothing(pool: PgPool) {
    setup_roles_over_the_previous_state(&pool).await;
    let mut reads = vec![
        "pools".to_string(),
        "archive_probe.kept".to_string(),
        "_timescaledb_catalog.hypertable".to_string(),
    ];
    reads.extend(born_after_the_file(&pool).await);

    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("SET ROLE yog_archive")
        .execute(&mut *conn)
        .await
        .unwrap();
    for relation in reads {
        sqlx::query(AssertSqlSafe(format!("SELECT count(*) FROM {relation}")))
            .execute(&mut *conn)
            .await
            .unwrap_or_else(|e| panic!("yog_archive must read {relation}: {e}"));
    }

    assert_reads_no_password_hash(&mut conn).await;

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
