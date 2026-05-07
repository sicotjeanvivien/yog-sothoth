use sqlx::PgPool;

use std::{env, sync::Arc};

#[allow(dead_code)]
pub(crate) struct Container {}
impl Container {
    pub(crate) async fn build() -> Self {
        Self {}
    }

    async fn init_db() -> PgPool {
        let database_url =
            env::var("DATABASE_URL_API").expect("DATABASE_URL_API must be set in .env");

        let pool = PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to PgPool");
        pool
    }
}
