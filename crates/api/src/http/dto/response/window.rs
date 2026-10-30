//! The envelope of a measure over a window: the window, the period it covers,
//! and the rows. ⚠️ A total whose period starts before `coveredSince` has no row.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::Window;

use crate::application::{Measured, PeriodBounds, SeriesInterval};

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
pub(in crate::http::dto::response) enum WindowResponse {
    #[serde(rename = "1h")]
    Hour,
    #[serde(rename = "24h")]
    Day,
    #[serde(rename = "7d")]
    Week,
    #[serde(rename = "30d")]
    Month,
}

impl From<Window> for WindowResponse {
    fn from(window: Window) -> Self {
        match window {
            Window::Hour => Self::Hour,
            Window::Day => Self::Day,
            Window::Week => Self::Week,
            Window::Month => Self::Month,
        }
    }
}

/// `from..asOf`, each `null` before any fact.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowedResponse<T> {
    window: WindowResponse,
    from: Option<DateTime<Utc>>,
    /// The instant the rows reflect: the close of the last frozen hour, or where `1h` reads tranches
    /// the end of the last one written.
    as_of: Option<DateTime<Utc>>,
    /// Where the facts start.
    covered_since: Option<DateTime<Utc>>,
    items: Vec<T>,
}

impl<T> WindowedResponse<T> {
    pub(crate) fn new<U, P: PeriodBounds>(window: Window, measured: Measured<Vec<U>, P>) -> Self
    where
        T: From<U>,
    {
        let period = measured.period;
        Self {
            window: window.into(),
            from: period.map(|s| s.from()),
            as_of: period.map(|s| s.as_of()),
            covered_since: period.map(|s| s.covered_since()),
            items: measured.items.into_iter().map(T::from).collect(),
        }
    }
}

/// The step of a series.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
pub(crate) enum IntervalResponse {
    #[serde(rename = "5m")]
    FiveMinutes,
    #[serde(rename = "1h")]
    Hour,
    #[serde(rename = "6h")]
    SixHours,
    #[serde(rename = "1d")]
    Day,
}

impl From<SeriesInterval> for IntervalResponse {
    fn from(interval: SeriesInterval) -> Self {
        match interval {
            SeriesInterval::FiveMinutes => Self::FiveMinutes,
            SeriesInterval::Hour => Self::Hour,
            SeriesInterval::SixHours => Self::SixHours,
            SeriesInterval::Day => Self::Day,
        }
    }
}

/// A measure by bucket: `interval` is the step of the buckets `from..asOf` hold.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SeriesResponse<T> {
    window: WindowResponse,
    interval: IntervalResponse,
    from: Option<DateTime<Utc>>,
    /// The instant the rows reflect: the close of the last frozen hour, or where `1h` reads tranches
    /// the end of the last one written.
    as_of: Option<DateTime<Utc>>,
    /// Where the facts start.
    covered_since: Option<DateTime<Utc>>,
    items: Vec<T>,
}

impl<T> SeriesResponse<T> {
    pub(crate) fn new<U, P: PeriodBounds>(
        window: Window,
        interval: SeriesInterval,
        measured: Measured<Vec<U>, P>,
    ) -> Self
    where
        T: From<U>,
    {
        let period = measured.period;
        Self {
            window: window.into(),
            interval: interval.into(),
            from: period.map(|s| s.from()),
            as_of: period.map(|s| s.as_of()),
            covered_since: period.map(|s| s.covered_since()),
            items: measured.items.into_iter().map(T::from).collect(),
        }
    }
}

/// What `poolHours` and `poolHoursValued` count: a pool's hours, or over `1h` its tranches.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CoverageUnitResponse {
    PoolHours,
    PoolTranches,
}

impl From<Window> for CoverageUnitResponse {
    fn from(window: Window) -> Self {
        match window {
            Window::Hour => Self::PoolTranches,
            Window::Day | Window::Week | Window::Month => Self::PoolHours,
        }
    }
}

/// The swaps by bucket: `interval` is their step, `coverageUnit` what their coverage counts.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SwapSeriesResponse<T> {
    window: WindowResponse,
    interval: IntervalResponse,
    coverage_unit: CoverageUnitResponse,
    from: Option<DateTime<Utc>>,
    /// The instant the rows reflect: the close of the last frozen hour, or where `1h` reads tranches
    /// the end of the last one written.
    as_of: Option<DateTime<Utc>>,
    /// Where the facts start.
    covered_since: Option<DateTime<Utc>>,
    items: Vec<T>,
}

impl<T> SwapSeriesResponse<T> {
    pub(crate) fn new<U, P: PeriodBounds>(
        window: Window,
        interval: SeriesInterval,
        measured: Measured<Vec<U>, P>,
    ) -> Self
    where
        T: From<U>,
    {
        let period = measured.period;
        Self {
            window: window.into(),
            interval: interval.into(),
            coverage_unit: window.into(),
            from: period.map(|s| s.from()),
            as_of: period.map(|s| s.as_of()),
            covered_since: period.map(|s| s.covered_since()),
            items: measured.items.into_iter().map(T::from).collect(),
        }
    }
}

/// The swaps over `from..asOf`: `coverageUnit` is what their coverage counts.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SwapTotalsResponse<T> {
    window: WindowResponse,
    coverage_unit: CoverageUnitResponse,
    from: Option<DateTime<Utc>>,
    /// The instant the rows reflect: the close of the last frozen hour, or where `1h` reads tranches
    /// the end of the last one written.
    as_of: Option<DateTime<Utc>>,
    /// Where the facts start.
    covered_since: Option<DateTime<Utc>>,
    items: Vec<T>,
}

impl<T> SwapTotalsResponse<T> {
    pub(crate) fn new<U, P: PeriodBounds>(window: Window, measured: Measured<Vec<U>, P>) -> Self
    where
        T: From<U>,
    {
        let period = measured.period;
        Self {
            window: window.into(),
            coverage_unit: window.into(),
            from: period.map(|s| s.from()),
            as_of: period.map(|s| s.as_of()),
            covered_since: period.map(|s| s.covered_since()),
            items: measured.items.into_iter().map(T::from).collect(),
        }
    }
}
