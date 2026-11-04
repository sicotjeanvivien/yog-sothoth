use yog_core::{RepositoryError, RepositoryResult, domain::LiveRecord};

/// Each row with its id, decoded and made an event of its collection by `wrap`.
/// ⚠️ A row that does not decode keeps its id: one bad row must not hold its table.
pub(crate) fn live_records<R, D, E>(
    rows: Vec<R>,
    id: impl Fn(&R) -> i64,
    wrap: impl Fn(D) -> E,
) -> Vec<LiveRecord<RepositoryResult<E>>>
where
    R: TryInto<D, Error = RepositoryError>,
{
    rows.into_iter()
        .map(|row| LiveRecord {
            id: id(&row),
            event: row.try_into().map(&wrap),
        })
        .collect()
}
