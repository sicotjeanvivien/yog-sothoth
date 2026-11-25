use async_trait::async_trait;

use super::{TokenTvlCursor, TokenTvlOrder, TokenTvlPage};
use crate::RepositoryResult;
use crate::domain::Protocol;

/// Read lens on yog-analytic's TVL by token, the api's.
#[async_trait]
pub trait TokenTvlLookup: Send + Sync {
    /// `protocol`'s tokens at its last reading, in `order`, after `cursor`'s key; forward only.
    /// `None` before the first reading. ⚠️ The reading and the rows are read together.
    async fn list(
        &self,
        protocol: Protocol,
        order: TokenTvlOrder,
        cursor: Option<TokenTvlCursor>,
        limit: i64,
    ) -> RepositoryResult<Option<TokenTvlPage>>;
}
