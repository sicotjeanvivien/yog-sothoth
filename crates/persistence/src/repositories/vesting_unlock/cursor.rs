use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use yog_core::{RepositoryResult, domain::VestingUnlockCursor};

use crate::repositories::helper::convert_decimal_to_bigdecimal;

/// A cursor's keys as the queries bind them, all `None` without one.
#[derive(Default)]
pub(super) struct CursorKeys {
    pub(super) given: bool,
    pub(super) amount_usd: Option<BigDecimal>,
    pub(super) unlocks_at: Option<DateTime<Utc>>,
    pub(super) pool_address: Option<String>,
    pub(super) position: Option<String>,
}

pub(super) fn keys_of(cursor: Option<&VestingUnlockCursor>) -> RepositoryResult<CursorKeys> {
    let Some(c) = cursor else {
        return Ok(CursorKeys::default());
    };
    Ok(CursorKeys {
        given: true,
        amount_usd: c
            .amount_usd
            .map(|v| convert_decimal_to_bigdecimal(v, "cursor amount_usd"))
            .transpose()?,
        unlocks_at: Some(c.unlocks_at),
        pool_address: Some(c.pool_address.to_string()),
        position: Some(c.position.to_string()),
    })
}
