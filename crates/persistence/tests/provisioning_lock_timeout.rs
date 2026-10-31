//! The `lock_timeout` of `yog_migrate` holds for its jobs and not for the migrations.

use sqlx::{ConnectOptions, PgPool};
use yog_persistence::Database;

use super::helpers::{CLUSTER_ROLES, apply_setup_roles};

async fn yog_migrate_settings(pool: &PgPool) -> (Vec<String>, String) {
    sqlx::query_as(
        "SELECT r.rolconfig, s.xmin::TEXT
           FROM pg_roles r JOIN pg_db_role_setting s ON s.setrole = r.oid AND s.setdatabase = 0
          WHERE r.rolname = 'yog_migrate'",
    )
    .fetch_one(pool)
    .await
    .expect("yog_migrate carries a setting")
}

/// Against removing the `ALTER ROLE`, then against removing its guard: an unconditional one
/// rewrites the row at every deploy, which the file promises not to do.
#[sqlx::test]
async fn setup_roles_bounds_the_lock_wait_of_yog_migrate_and_a_rerun_writes_nothing(pool: PgPool) {
    apply_setup_roles(&pool).await;
    // ⚠️ Roles outlive the test's database: without this, an earlier run's setting answers.
    {
        let _cluster = CLUSTER_ROLES.lock().await;
        sqlx::query("ALTER ROLE yog_migrate RESET lock_timeout")
            .execute(&pool)
            .await
            .unwrap();
    }

    apply_setup_roles(&pool).await;
    let (config, written_by) = yog_migrate_settings(&pool).await;
    assert!(config.contains(&"lock_timeout=3s".to_owned()), "{config:?}");

    apply_setup_roles(&pool).await;
    assert_eq!(yog_migrate_settings(&pool).await.1, written_by);
}

/// Against removing the startup parameter of `connect_for_provisioning`. ⚠️ On a role without
/// the setting this would pass regardless, 0 being the default: the plain connection proves it.
/// A login role of its own: `yog_migrate`'s password is not the test's to know.
#[sqlx::test]
async fn a_provisioning_connection_lifts_the_lock_timeout_of_its_role(pool: PgPool) {
    let _cluster = CLUSTER_ROLES.lock().await;
    sqlx::raw_sql(
        "DROP ROLE IF EXISTS yog_probe_lock_timeout;
         CREATE ROLE yog_probe_lock_timeout LOGIN PASSWORD 'probe';
         ALTER ROLE yog_probe_lock_timeout SET lock_timeout = '3s';",
    )
    .execute(&pool)
    .await
    .unwrap();
    let options = (*pool.connect_options())
        .clone()
        .username("yog_probe_lock_timeout")
        .password("probe");

    let plain = PgPool::connect_with(options.clone()).await.unwrap();
    let inherited: String = sqlx::query_scalar("SHOW lock_timeout")
        .fetch_one(&plain)
        .await
        .unwrap();
    plain.close().await;

    let database = Database::connect_for_provisioning(options.to_url_lossy().as_str())
        .await
        .unwrap();
    let lifted: String = sqlx::query_scalar("SHOW lock_timeout")
        .fetch_one(database.pool())
        .await
        .unwrap();
    database.close().await;

    sqlx::raw_sql("DROP ROLE yog_probe_lock_timeout")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(inherited, "3s");
    assert_eq!(lifted, "0");
}
