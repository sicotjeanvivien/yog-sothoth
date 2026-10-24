//! The envelope of a reading at an instant, the TVL's: no window, the instant it reflects and
//! the facts' coverage, then one reading per row.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use crate::application::Measured;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReadingResponse<T> {
    /// The close of the last frozen hour; a row's own `at` may be earlier.
    as_of: Option<DateTime<Utc>>,
    covered_since: Option<DateTime<Utc>>,
    items: Vec<T>,
}

impl<T> ReadingResponse<T> {
    pub(crate) fn new<U>(measured: Measured<Vec<U>>) -> Self
    where
        T: From<U>,
    {
        Self {
            as_of: measured.span.map(|s| s.as_of),
            covered_since: measured.span.map(|s| s.covered_since),
            items: measured.items.into_iter().map(T::from).collect(),
        }
    }
}
