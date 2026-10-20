use async_trait::async_trait;

use super::{TokenActivityCursor, TokenActivityPage};
use crate::RepositoryResult;
use crate::domain::Window;

/// Read lens on yog-analytic's aggregate by token, the api's.
#[async_trait]
pub trait TokenActivityFeed: Send + Sync {
    /// The last ranking written for `window`, by volume descending then mint,
    /// after `cursor`'s key; forward only. `None` before the first freeze.
    /// ⚠️ The hour and the rows are read together: a freeze cannot fall between them.
    async fn list(
        &self,
        window: Window,
        cursor: Option<TokenActivityCursor>,
        limit: i64,
    ) -> RepositoryResult<Option<TokenActivityPage>>;
}
