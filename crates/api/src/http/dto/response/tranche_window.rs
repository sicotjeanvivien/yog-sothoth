//! The envelope of a measure through the last tranche written: the window, the period it
//! covers, and the rows. ⚠️ A total whose period starts before `coveredSince` has no row.

use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{TranchePeriod, Window};

use super::window::{WindowPeriodResponse, WindowResponse};
use crate::application::Measured;

/// A measure over its window, through the last tranche written.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrancheWindowedResponse<T> {
    window: WindowResponse,
    #[serde(flatten)]
    period: WindowPeriodResponse,
    items: Vec<T>,
}

impl<T> TrancheWindowedResponse<T> {
    pub(crate) fn new<U>(window: Window, measured: Measured<Vec<U>, TranchePeriod>) -> Self
    where
        T: From<U>,
    {
        Self {
            window: window.into(),
            period: WindowPeriodResponse::of(window, measured.period),
            items: measured.items.into_iter().map(T::from).collect(),
        }
    }
}
