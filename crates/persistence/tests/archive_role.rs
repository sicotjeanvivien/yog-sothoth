//! The `yog_archive` role: it reads every schema `pg_dump` reads, chunks born
//! after `setup_roles.sql` included, and writes nothing — nor reads `pg_authid`.
//! Those grants come from that file, not from a migration, so `privileges.rs`
//! cannot see them: these tests apply it, then act under the role.

use super::helpers::{INSUFFICIENT_PRIVILEGE, apply_setup_roles, sqlstate};
use sqlx::{AssertSqlSafe, PgPool};
use yog_persistence::PgServerInfo;

/// A price at `fetched_at`, in whatever chunk of `token_prices` holds it.
async fn price_at(pool: &PgPool, fetched_at: &str) {
    sqlx::query(
        "INSERT INTO token_prices (mint, price_usd, price_provider, fetched_at)
         VALUES ('archive_probe', 1, 'jupiter', $1::TIMESTAMPTZ)",
    )
    .bind(fetched_at)
    .execute(pool)
    .await
    .unwrap();
}

/// The chunk born for a price far in the future, and the compressed chunk of
/// an old one — both after the file ran, as production makes them between two
/// deploys.
async fn chunks_born_after_the_file(pool: &PgPool) -> [String; 2] {
    price_at(pool, "2030-01-01").await;
    price_at(pool, "2020-01-01").await;
    let new_chunk: String = sqlx::query_scalar(
        "SELECT c::TEXT FROM show_chunks('token_prices', newer_than => TIMESTAMPTZ '2029-12-01') c",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let old_chunk: String = sqlx::query_scalar(
        "SELECT compress_chunk(c)::TEXT
           FROM show_chunks('token_prices', older_than => TIMESTAMPTZ '2020-02-01') c",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let compressed: String = sqlx::query_scalar(
        "SELECT compress_relid::TEXT FROM _timescaledb_catalog.compression_settings
          WHERE relid = $1::REGCLASS",
    )
    .bind(&old_chunk)
    .fetch_one(pool)
    .await
    .unwrap();
    [new_chunk, compressed]
}

/// Against the per-schema grants of the file removed or narrowed to `public`,
/// and against `pg_read_all_data` given back, which reads `pg_authid`.
#[sqlx::test]
async fn yog_archive_reads_every_schema_and_writes_nothing(pool: PgPool) {
    sqlx::raw_sql("CREATE SCHEMA archive_probe; CREATE TABLE archive_probe.kept (id int);")
        .execute(&pool)
        .await
        .unwrap();
    apply_setup_roles(&pool).await;
    let born_after = chunks_born_after_the_file(&pool).await;
    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("SET ROLE yog_archive")
        .execute(&mut *conn)
        .await
        .unwrap();

    let mut reads = vec![
        "SELECT count(*) FROM pools".to_string(),
        "SELECT count(*) FROM archive_probe.kept".to_string(),
        "SELECT count(*) FROM _timescaledb_catalog.hypertable".to_string(),
    ];
    reads.extend(
        born_after
            .iter()
            .map(|chunk| format!("SELECT count(*) FROM {chunk}")),
    );
    for read in reads {
        sqlx::query(AssertSqlSafe(read.clone()))
            .execute(&mut *conn)
            .await
            .unwrap_or_else(|e| panic!("yog_archive must run `{read}`: {e}"));
    }

    let err = sqlx::query("SELECT rolpassword FROM pg_authid")
        .execute(&mut *conn)
        .await
        .expect_err("yog_archive reads no password hash");
    assert_eq!(sqlstate(&err), INSUFFICIENT_PRIVILEGE);
    let message = err.as_database_error().unwrap().message().to_owned();
    assert!(message.contains("table pg_authid"), "{message}");

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
