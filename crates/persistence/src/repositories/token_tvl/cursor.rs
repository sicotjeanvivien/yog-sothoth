use bigdecimal::BigDecimal;
use yog_core::{RepositoryResult, domain::TokenTvlCursor};

use crate::repositories::helper::convert_decimal_to_bigdecimal;

/// A cursor's keys as the queries bind them, all `None` without one.
#[derive(Default)]
pub(super) struct CursorKeys {
    pub(super) given: bool,
    pub(super) value: Option<BigDecimal>,
    pub(super) mint: Option<String>,
}

pub(super) fn keys_of(cursor: Option<&TokenTvlCursor>) -> RepositoryResult<CursorKeys> {
    let Some(c) = cursor else {
        return Ok(CursorKeys::default());
    };
    Ok(CursorKeys {
        given: true,
        value: c
            .value
            .map(|v| convert_decimal_to_bigdecimal(v, "cursor value"))
            .transpose()?,
        mint: Some(c.mint.to_string()),
    })
}
