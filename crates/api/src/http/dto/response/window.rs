//! The envelope of a measure over a window: the window, the period it covers,
//! and the rows. ⚠️ A total whose period starts before `coveredSince` has no row.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{
    LiquidityTurnover, ProtocolLiquidityTurnover, ProtocolPoolCreations, ProtocolSwapActivity,
    ProtocolTvlChange, SwapActivity, SwapActivityTotal, SwapBucket, TvlChangeTotal, Window,
};

use super::protocol::ProtocolResponse;
use super::swap_activity::SwapActivityResponse;
use crate::application::{Availability, Measured, PeriodBounds, SeriesInterval, SwapTotals};
use crate::http::dto::schema::{Count, SignedDecimal, UnsignedDecimal};

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
    /// The instant the rows reflect.
    as_of: Option<DateTime<Utc>>,
    /// Where the read's coverage starts.
    covered_since: Option<DateTime<Utc>>,
    /// Whether the service opens this window: it is covered whole, and not held closed.
    open: bool,
    /// When it will be covered whole; `null` when open, or when no date can be said.
    opens_at: Option<DateTime<Utc>>,
    /// Whether `asOf` is further behind than a frozen hour or a tranche should be.
    late: bool,
}

impl WindowPeriodResponse {
    pub(crate) fn of<P: PeriodBounds>(window: Window, period: Option<P>) -> Self {
        Self::moved(window, None, period)
    }

    fn moved<P: PeriodBounds>(window: Window, offset: Option<Window>, period: Option<P>) -> Self {
        let availability = Availability::of(window, offset, period, Utc::now());
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
    pub(crate) fn new<U, P: PeriodBounds>(
        window: Window,
        offset: Option<Window>,
        measured: Measured<Vec<U>, P>,
    ) -> Self
    where
        T: From<U>,
    {
        Self {
            window: window.into(),
            coverage_unit: window.into(),
            period: WindowPeriodResponse::moved(window, offset, measured.period),
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

/// The protocols of a read summed; each field unknown as soon as one protocol's is.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SwapTotalResponse {
    #[serde(flatten)]
    activity: SwapActivityResponse,
    #[schema(value_type = Option<Count>)]
    pools: Option<i64>,
}

impl From<SwapActivityTotal> for SwapTotalResponse {
    fn from(total: SwapActivityTotal) -> Self {
        Self {
            activity: total.activity.into(),
            pools: total.pools,
        }
    }
}

/// The window before this one: whether the facts cover it whole, and the volume over it.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreviousVolumeResponse {
    covered: bool,
    #[schema(value_type = Option<UnsignedDecimal>)]
    volume_usd: Option<Decimal>,
}

/// [`SwapTotalsResponse`] of the protocols: their `total`, and the window before it; `previous` is
/// `null` for a read moved back by `offset`, and before any fact.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolSwapTotalsResponse<T> {
    window: WindowResponse,
    coverage_unit: CoverageUnitResponse,
    #[serde(flatten)]
    period: WindowPeriodResponse,
    total: Option<SwapTotalResponse>,
    previous: Option<PreviousVolumeResponse>,
    items: Vec<T>,
}

impl<T: From<ProtocolSwapActivity>> ProtocolSwapTotalsResponse<T> {
    pub(crate) fn new(window: Window, offset: Option<Window>, totals: SwapTotals) -> Self {
        let measured = totals.measured;
        Self {
            window: window.into(),
            coverage_unit: window.into(),
            period: WindowPeriodResponse::moved(window, offset, measured.period),
            total: ProtocolSwapActivity::total(&measured.items).map(Into::into),
            previous: totals.previous.map(|previous| PreviousVolumeResponse {
                covered: previous.covered,
                volume_usd: previous.volume_usd,
            }),
            items: measured.items.into_iter().map(T::from).collect(),
        }
    }
}

/// Every protocol's swaps over one bucket.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SwapBucketTotalResponse {
    from: DateTime<Utc>,
    #[serde(flatten)]
    activity: SwapActivityResponse,
}

impl From<SwapBucket> for SwapBucketTotalResponse {
    fn from(bucket: SwapBucket) -> Self {
        Self {
            from: bucket.from,
            activity: bucket.activity.into(),
        }
    }
}

/// [`SwapSeriesResponse`] of the protocols: `total` sums each bucket's, a USD field over the
/// protocols that know it, the earliest first.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolSwapSeriesResponse<T> {
    window: WindowResponse,
    interval: IntervalResponse,
    coverage_unit: CoverageUnitResponse,
    #[serde(flatten)]
    period: WindowPeriodResponse,
    total: Vec<SwapBucketTotalResponse>,
    items: Vec<T>,
}

impl<T> ProtocolSwapSeriesResponse<T> {
    pub(crate) fn new<P: PeriodBounds>(
        window: Window,
        interval: SeriesInterval,
        measured: Measured<Vec<yog_core::domain::ProtocolBucket<SwapActivity>>, P>,
    ) -> Self
    where
        T: From<yog_core::domain::ProtocolBucket<SwapActivity>>,
    {
        Self {
            window: window.into(),
            interval: interval.into(),
            coverage_unit: window.into(),
            period: WindowPeriodResponse::of(window, measured.period),
            total: SwapActivity::bucket_totals(&measured.items)
                .into_iter()
                .map(Into::into)
                .collect(),
            items: measured.items.into_iter().map(T::from).collect(),
        }
    }
}

/// The protocols' TVL over a window, summed; each field unknown as soon as one protocol's is.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TvlTotalResponse {
    #[schema(value_type = Option<UnsignedDecimal>)]
    end_tvl_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    start_tvl_usd: Option<Decimal>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    discovered_tvl_usd: Option<Decimal>,
    #[schema(value_type = Option<SignedDecimal>)]
    change_usd: Option<Decimal>,
}

impl From<TvlChangeTotal> for TvlTotalResponse {
    fn from(total: TvlChangeTotal) -> Self {
        Self {
            end_tvl_usd: total.end_tvl_usd,
            start_tvl_usd: total.start_tvl_usd,
            discovered_tvl_usd: total.discovered_tvl_usd,
            change_usd: total.change_usd,
        }
    }
}

/// [`WindowedResponse`] of the TVL's change, with its `total`.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TvlTotalsResponse<T> {
    window: WindowResponse,
    #[serde(flatten)]
    period: WindowPeriodResponse,
    total: Option<TvlTotalResponse>,
    items: Vec<T>,
}

impl<T: From<ProtocolTvlChange>> TvlTotalsResponse<T> {
    pub(crate) fn new<P: PeriodBounds>(
        window: Window,
        measured: Measured<Vec<ProtocolTvlChange>, P>,
    ) -> Self {
        Self {
            window: window.into(),
            period: WindowPeriodResponse::of(window, measured.period),
            total: ProtocolTvlChange::total(&measured.items).map(Into::into),
            items: measured.items.into_iter().map(T::from).collect(),
        }
    }
}

/// The protocols' pools created over a window, summed.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolCreationsTotalResponse {
    #[schema(value_type = Count)]
    pools: i64,
}

/// [`WindowedResponse`] of the pools created, with their `total`.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolCreationTotalsResponse<T> {
    window: WindowResponse,
    #[serde(flatten)]
    period: WindowPeriodResponse,
    total: Option<PoolCreationsTotalResponse>,
    items: Vec<T>,
}

impl<T: From<ProtocolPoolCreations>> PoolCreationTotalsResponse<T> {
    pub(crate) fn new<P: PeriodBounds>(
        window: Window,
        measured: Measured<Vec<ProtocolPoolCreations>, P>,
    ) -> Self {
        Self {
            window: window.into(),
            period: WindowPeriodResponse::of(window, measured.period),
            total: ProtocolPoolCreations::total(&measured.items)
                .map(|pools| PoolCreationsTotalResponse { pools }),
            items: measured.items.into_iter().map(T::from).collect(),
        }
    }
}

/// How many times the liquidity deposited changed hands over the window, its coverage that of the
/// volume; `turnover` is `null` without a volume, or a TVL above 0.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiquidityTurnoverResponse {
    #[schema(value_type = Option<UnsignedDecimal>)]
    turnover: Option<Decimal>,
    #[schema(value_type = Count)]
    pool_hours: i64,
    #[schema(value_type = Count)]
    pool_hours_valued: i64,
}

impl From<LiquidityTurnover> for LiquidityTurnoverResponse {
    fn from(turnover: LiquidityTurnover) -> Self {
        Self {
            turnover: turnover.turnover,
            pool_hours: turnover.pool_hours,
            pool_hours_valued: turnover.pool_hours_valued,
        }
    }
}

/// A row of `liquidity-turnover/totals`.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolLiquidityTurnoverResponse {
    protocol: ProtocolResponse,
    #[serde(flatten)]
    turnover: LiquidityTurnoverResponse,
    /// The close of the hour of the TVL reading divided by; `null` without one.
    tvl_at: Option<DateTime<Utc>>,
}

impl From<ProtocolLiquidityTurnover> for ProtocolLiquidityTurnoverResponse {
    fn from(row: ProtocolLiquidityTurnover) -> Self {
        Self {
            protocol: row.protocol.into(),
            turnover: row.turnover.into(),
            tvl_at: row.tvl_at,
        }
    }
}

/// `liquidity-turnover/totals`: `total` is every volume over every TVL, not a sum of the rows'.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiquidityTurnoverTotalsResponse {
    window: WindowResponse,
    coverage_unit: CoverageUnitResponse,
    #[serde(flatten)]
    period: WindowPeriodResponse,
    total: Option<LiquidityTurnoverResponse>,
    items: Vec<ProtocolLiquidityTurnoverResponse>,
}

impl LiquidityTurnoverTotalsResponse {
    pub(crate) fn new<P: PeriodBounds>(
        window: Window,
        measured: Measured<Vec<ProtocolLiquidityTurnover>, P>,
        total: Option<LiquidityTurnover>,
    ) -> Self {
        Self {
            window: window.into(),
            coverage_unit: window.into(),
            period: WindowPeriodResponse::of(window, measured.period),
            total: total.map(Into::into),
            items: measured.items.into_iter().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
#[path = "tests/window_tests.rs"]
mod tests;
