use async_trait::async_trait;

use super::{VestingUnlockCursor, VestingUnlockOrder, VestingUnlockPage};
use crate::RepositoryResult;
use crate::domain::{Protocol, Window};

/// Read lens on yog-analytic's vesting releases, the api's.
#[async_trait]
pub trait VestingUnlockLookup: Send + Sync {
    /// `protocol`'s releases within `window` after its last reading, in `order`, after
    /// `cursor`'s key; forward only. `None` before the first reading.
    async fn list(
        &self,
        protocol: Protocol,
        window: Window,
        order: VestingUnlockOrder,
        cursor: Option<VestingUnlockCursor>,
        limit: i64,
    ) -> RepositoryResult<Option<VestingUnlockPage>>;
}
