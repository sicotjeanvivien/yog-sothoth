use yog_core::{
    Cursor, Page, RepositoryResult,
    domain::{Protocol, TokenTvlCursor, TokenTvlOrder, TokenTvlPage, TokenTvlTotal},
};

use super::rows::TokenTvlRow;
use crate::repositories::helper::{
    PageBuilder, QueryMode, convert_bigdecimal_to_decimal, convert_optional,
};

/// The page of `rows`, read in `order`; `None` before the first reading.
pub(super) fn page_of(
    rows: Vec<TokenTvlRow>,
    protocol: Protocol,
    order: TokenTvlOrder,
    had_cursor: bool,
    limit: i64,
) -> RepositoryResult<Option<TokenTvlPage>> {
    let Some(first) = rows.first() else {
        return Ok(None);
    };
    let Some(at) = first.at else {
        return Ok(None);
    };
    let covered_since = first.covered_since;
    let total = TokenTvlTotal {
        tvl_usd: convert_optional(
            first.total_tvl_usd.clone(),
            "total_tvl_usd",
            convert_bigdecimal_to_decimal,
        )?,
        locked_usd: convert_optional(
            first.total_locked_usd.clone(),
            "total_locked_usd",
            convert_bigdecimal_to_decimal,
        )?,
    };
    let keyed = rows
        .into_iter()
        .filter_map(|row| row.into_token().transpose())
        .collect::<RepositoryResult<Vec<_>>>()?;
    let page =
        PageBuilder::new(keyed, limit, QueryMode::Forward, had_cursor).finalize(|(token, key)| {
            Cursor::TokenTvl(Box::new(TokenTvlCursor {
                protocol,
                at,
                order,
                value: *key,
                mint: token.mint,
            }))
        });
    // ⚠️ Forward only: a previous cursor would be read forward from the first item.
    Ok(Some(TokenTvlPage {
        at,
        covered_since,
        total,
        page: Page {
            items: page.items.into_iter().map(|(token, _)| token).collect(),
            next_cursor: page.next_cursor,
            prev_cursor: None,
            is_first: page.is_first,
            is_last: page.is_last,
        },
    }))
}
