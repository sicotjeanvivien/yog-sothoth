use yog_core::{PageDirection, PagePosition, PoolSort, domain::PoolCursor};

/// Input parameters for `PoolService::list_pools`.
///
/// All fields are domain types — the HTTP layer is responsible for
/// parsing query params, decoding the cursor, converting wire enums,
/// and normalizing the search term before constructing this.
pub(crate) struct PoolListParams {
    pub(crate) cursor: Option<PoolCursor>,
    pub(crate) direction: PageDirection,
    pub(crate) position: Option<PagePosition>,
    pub(crate) sort: PoolSort,
    pub(crate) search: Option<String>,
    pub(crate) limit: i64,
}
