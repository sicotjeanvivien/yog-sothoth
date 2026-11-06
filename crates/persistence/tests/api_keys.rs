//! `PgApiKeyRepository`: the operator's writes, and `active_keys` read under `yog_api`, which
//! may read the keys and write none.

use sqlx::PgPool;
use yog_core::{
    RepositoryError,
    domain::{ApiKey, ApiKeyLookup},
};
use yog_persistence::PgApiKeyRepository;

use super::helpers::{
    CHECK_VIOLATION, INSUFFICIENT_PRIVILEGE, setup_roles_and_connect_as, sqlstate,
};

fn key(byte: u8) -> ApiKey {
    ApiKey::from_random([byte; 4], [byte; 32])
}

/// Mutation: either `revoked_at IS NULL` dropped, so a revoked key, or a revoked client's, opens.
#[sqlx::test(migrations = "./migrations")]
async fn the_active_keys_are_the_unrevoked_keys_of_unrevoked_clients(pool: PgPool) {
    let operator = PgApiKeyRepository::new(pool.clone());
    let (kept, revoked, of_revoked_client) = (key(1), key(2), key(3));
    operator.create("dashboard", &kept).await.unwrap();
    operator.create("dashboard", &revoked).await.unwrap();
    operator.create("gone", &of_revoked_client).await.unwrap();
    assert!(operator.revoke(revoked.prefix()).await.unwrap());
    sqlx::query("UPDATE api_clients SET revoked_at = now() WHERE name = 'gone'")
        .execute(&pool)
        .await
        .unwrap();

    let api = PgApiKeyRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let active = api.active_keys().await.unwrap();

    assert_eq!(active.len(), 1, "{active:?}");
    assert_eq!(active[0].hash, kept.hash());
    assert_eq!(active[0].client.name, "dashboard");
}

/// Mutation: the key itself stored, in any column.
#[sqlx::test(migrations = "./migrations")]
async fn the_table_keeps_the_hash_and_the_prefix_never_the_key(pool: PgPool) {
    let issued = key(7);
    PgApiKeyRepository::new(pool.clone())
        .create("dashboard", &issued)
        .await
        .unwrap();

    let (prefix, hash, row): (String, Vec<u8>, String) =
        sqlx::query_as("SELECT prefix, key_hash, k::TEXT FROM api_keys k")
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(prefix, issued.prefix());
    assert_eq!(hash, issued.hash());
    assert!(
        !row.contains(&issued.expose()[issued.expose().len() - 64..]),
        "the secret must be nowhere in the row: {row}"
    );
}

/// Mutation: the revoked-client guard removed, so a key is stored that would never open.
#[sqlx::test(migrations = "./migrations")]
async fn a_revoked_client_is_issued_no_key(pool: PgPool) {
    let operator = PgApiKeyRepository::new(pool.clone());
    operator.create("gone", &key(1)).await.unwrap();
    sqlx::query("UPDATE api_clients SET revoked_at = now() WHERE name = 'gone'")
        .execute(&pool)
        .await
        .unwrap();

    let refused = operator.create("gone", &key(2)).await;

    assert!(
        matches!(&refused, Err(RepositoryError::Conflict(reason)) if reason.contains("revoked")),
        "{refused:?}"
    );
    let stored: i64 = sqlx::query_scalar("SELECT count(*) FROM api_keys")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, 1);
}

/// Mutation: `revoke` answering `true` whatever it touched.
#[sqlx::test(migrations = "./migrations")]
async fn revoking_says_whether_a_key_was_still_active(pool: PgPool) {
    let operator = PgApiKeyRepository::new(pool.clone());
    let issued = key(4);
    operator.create("dashboard", &issued).await.unwrap();

    assert!(operator.revoke(issued.prefix()).await.unwrap());
    assert!(
        !operator.revoke(issued.prefix()).await.unwrap(),
        "already revoked"
    );
    assert!(
        !operator.revoke("ffffffff").await.unwrap(),
        "no such prefix"
    );

    let listed = operator.list().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].revoked_at.is_some());
}

/// The privilege the matrix declares, exercised: `yog_api` reads the keys and writes none.
#[sqlx::test(migrations = "./migrations")]
async fn yog_api_cannot_write_a_key(pool: PgPool) {
    let api = setup_roles_and_connect_as(&pool, "yog_api").await;

    let refused = sqlx::query("INSERT INTO api_clients (name) VALUES ('intruder')")
        .execute(&api)
        .await
        .expect_err("yog_api may write nothing");

    assert_eq!(sqlstate(&refused), INSUFFICIENT_PRIVILEGE);
    assert!(refused.to_string().contains("api_clients"), "{refused}");
}

/// Mutation: the CHECK on `api_clients.name` dropped, so a name no metrics filter prints is stored.
#[sqlx::test(migrations = "./migrations")]
async fn a_client_name_outside_its_alphabet_is_refused_by_the_table(pool: PgPool) {
    let refused = PgApiKeyRepository::new(pool.clone())
        .create("Acme Corp", &key(1))
        .await;

    assert!(
        matches!(refused, Err(RepositoryError::Backend(_))),
        "{refused:?}"
    );
    let refused = sqlx::query("INSERT INTO api_clients (name) VALUES ('Acme Corp')")
        .execute(&pool)
        .await
        .expect_err("outside [a-z0-9-]");
    assert_eq!(sqlstate(&refused), CHECK_VIOLATION);
}
