use sqlx::PgPool;

pub(crate) struct PgLiquidityEventRepository {
    pool: PgPool,
}

impl PgLiquidityEventRepository {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
