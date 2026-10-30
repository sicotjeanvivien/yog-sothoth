//! The envelope of a measure over a window: the window, the period it covers,
//! and the rows. ⚠️ A total whose period starts before `coveredSince` has no row.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::Window;

use crate::application::{Availability, Measured, PeriodBounds, SeriesInterval};

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

/// What a response says of its window: its bounds, whether it is open, and whether it runs late.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowPeriodResponse {
    /// Where the window starts, `null` before any fact.
    from: Option<DateTime<Utc>>,
    /// The instant the rows reflect: the close of the last frozen hour, or where `1h` reads tranches
    /// the end of the last one written.
    as_of: Option<DateTime<Utc>>,
    /// Where the facts start.
    covered_since: Option<DateTime<Utc>>,
    /// Whether the service opens this window: the facts cover it whole, and it is not held closed.
    open: bool,
    /// When the facts will cover it; `null` when open, or when no date can be said.
    opens_at: Option<DateTime<Utc>>,
    /// Whether `asOf` is further behind than a frozen hour or a tranche should be.
    late: bool,
}

impl WindowPeriodResponse {
    pub(crate) fn of<P: PeriodBounds>(window: Window, period: Option<P>) -> Self {
        let availability = Availability::of(window, period, Utc::now());
        Self {
            from: period.map(|s| s.from()),
            as_of: period.map(|s| s.as_of()),
            covered_since: period.map(|s| s.covered_since()),
            open: availability.open,
            opens_at: availability.opens_at,
            late: availability.late,
        }
    }
}

/// A measure over its window.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowedResponse<T> {
    window: WindowResponse,
    #[serde(flatten)]
    period: WindowPeriodResponse,
    items: Vec<T>,
}

impl<T> WindowedResponse<T> {
    pub(crate) fn new<U, P: PeriodBounds>(window: Window, measured: Measured<Vec<U>, P>) -> Self
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
    #[serde(flatten)]
    period: WindowPeriodResponse,
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
        Self {
            window: window.into(),
            interval: interval.into(),
            period: WindowPeriodResponse::of(window, measured.period),
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
    #[serde(flatten)]
    period: WindowPeriodResponse,
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
        Self {
            window: window.into(),
            interval: interval.into(),
            coverage_unit: window.into(),
            period: WindowPeriodResponse::of(window, measured.period),
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
    #[serde(flatten)]
    period: WindowPeriodResponse,
    items: Vec<T>,
}

impl<T> SwapTotalsResponse<T> {
    pub(crate) fn new<U, P: PeriodBounds>(window: Window, measured: Measured<Vec<U>, P>) -> Self
    where
        T: From<U>,
    {
        Self {
            window: window.into(),
            coverage_unit: window.into(),
            period: WindowPeriodResponse::of(window, measured.period),
            items: measured.items.into_iter().map(T::from).collect(),
        }
    }
}

/// [`SeriesResponse`] of the volume the runs split, which says since when they split it.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunSplitSeriesResponse<T> {
    window: WindowResponse,
    interval: IntervalResponse,
    #[serde(flatten)]
    period: WindowPeriodResponse,
    /// The first instant the runs split a bar from; `null` before they are measured.
    split_since: Option<DateTime<Utc>>,
    items: Vec<T>,
}

impl<T> RunSplitSeriesResponse<T> {
    pub(crate) fn new<U, P: PeriodBounds>(
        window: Window,
        interval: SeriesInterval,
        measured: Measured<Vec<U>, P>,
        split_since: Option<DateTime<Utc>>,
    ) -> Self
    where
        T: From<U>,
    {
        Self {
            window: window.into(),
            interval: interval.into(),
            period: WindowPeriodResponse::of(window, measured.period),
            split_since,
            items: measured.items.into_iter().map(T::from).collect(),
        }
    }
}

/// The windows the service reads, each with what it says of its period.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowRowResponse {
    window: WindowResponse,
    #[serde(flatten)]
    period: WindowPeriodResponse,
}

impl<P: PeriodBounds> From<(Window, Option<P>)> for WindowRowResponse {
    fn from((window, period): (Window, Option<P>)) -> Self {
        Self {
            window: window.into(),
            period: WindowPeriodResponse::of(window, period),
        }
    }
}

/// Every window, the shortest first.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowsResponse {
    items: Vec<WindowRowResponse>,
}

impl WindowsResponse {
    pub(crate) fn new<P: PeriodBounds>(windows: Vec<(Window, Option<P>)>) -> Self {
        Self {
            items: windows.into_iter().map(Into::into).collect(),
        }
    }
}
