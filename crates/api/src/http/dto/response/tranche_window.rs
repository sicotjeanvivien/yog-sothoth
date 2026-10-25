//! The envelope of a measure through the last tranche written: the window, the period it
//! covers, and the rows. ⚠️ A total whose period starts before `coveredSince` has no row.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{TranchePeriod, Window};

use super::window::WindowResponse;
use crate::application::Measured;

/// `from..asOf`, each `null` before the first tranche is written.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrancheWindowedResponse<T> {
    window: WindowResponse,
    from: Option<DateTime<Utc>>,
    /// The end of the last tranche written: the instant the rows reflect.
    as_of: Option<DateTime<Utc>>,
    /// The first tranche the measure holds.
    covered_since: Option<DateTime<Utc>>,
    items: Vec<T>,
}

impl<T> TrancheWindowedResponse<T> {
    pub(crate) fn new<U>(window: Window, measured: Measured<Vec<U>, TranchePeriod>) -> Self
    where
        T: From<U>,
    {
        let period = measured.period;
        Self {
            window: window.into(),
            from: period.map(|s| s.from),
            as_of: period.map(|s| s.as_of),
            covered_since: period.map(|s| s.covered_since),
            items: measured.items.into_iter().map(T::from).collect(),
        }
    }
}
