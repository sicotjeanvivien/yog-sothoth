//! A protocol's TVL by token, at its last reading.

use std::sync::Arc;

use yog_core::{
    RepositoryResult,
    domain::{Protocol, TokenTvlCursor, TokenTvlLookup, TokenTvlOrder, TokenTvlPage},
};

/// A page of the ranking, or a cursor taken in a reading replaced since.
pub(crate) enum TokenTvlRead {
    /// `None` before the first reading.
    Page(Option<Box<TokenTvlPage>>),
    CursorExpired,
}

pub(crate) struct TokenTvlService {
    token_tvl: Arc<dyn TokenTvlLookup>,
}

impl TokenTvlService {
    pub(crate) fn new(token_tvl: Arc<dyn TokenTvlLookup>) -> Self {
        Self { token_tvl }
    }

    /// ⚠️ A cursor names its reading: once replaced, its keys rank another list.
    pub(crate) async fn list_by_token(
        &self,
        protocol: Protocol,
        order: TokenTvlOrder,
        cursor: Option<TokenTvlCursor>,
        limit: i64,
    ) -> RepositoryResult<TokenTvlRead> {
        let taken_at = cursor.as_ref().map(|c| c.at);
        let read = self.token_tvl.list(protocol, order, cursor, limit).await?;
        Ok(match (taken_at, &read) {
            (Some(at), Some(page)) if page.at != at => TokenTvlRead::CursorExpired,
            (Some(_), None) => TokenTvlRead::CursorExpired,
            _ => TokenTvlRead::Page(read.map(Box::new)),
        })
    }
}

#[cfg(test)]
#[path = "tests/token_tvl_service_tests.rs"]
mod tests;
