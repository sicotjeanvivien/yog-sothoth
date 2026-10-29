//! The `window` and `offset` of a measure: the closed list of `Window`.

use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use yog_core::domain::Window;

use crate::application::SeriesInterval;
use crate::http::error::ApiError;

#[derive(Debug, Default, Deserialize, Clone, Copy, ToSchema)]
pub(crate) enum WindowParam {
    #[default]
    #[serde(rename = "24h")]
    Day,
    #[serde(rename = "7d")]
    Week,
    #[serde(rename = "30d")]
    Month,
}

impl From<WindowParam> for Window {
    fn from(value: WindowParam) -> Self {
        match value {
            WindowParam::Day => Window::Day,
            WindowParam::Week => Window::Week,
            WindowParam::Month => Window::Month,
        }
    }
}

/// A measure over a window ending at the last frozen hour; 24 hours by default.
#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
pub(crate) struct WindowParams {
    #[serde(default)]
    #[param(inline)]
    pub(crate) window: WindowParam,
}

/// The window of a measure the Overview reads: [`WindowParam`]'s, and `1h`, the last hour through
/// the last tranche written.
#[derive(Debug, Default, Deserialize, Clone, Copy, ToSchema)]
pub(crate) enum MeasureWindowParam {
    #[serde(rename = "1h")]
    Hour,
    #[default]
    #[serde(rename = "24h")]
    Day,
    #[serde(rename = "7d")]
    Week,
    #[serde(rename = "30d")]
    Month,
}

impl From<MeasureWindowParam> for Window {
    fn from(value: MeasureWindowParam) -> Self {
        match value {
            MeasureWindowParam::Hour => Window::Hour,
            MeasureWindowParam::Day => Window::Day,
            MeasureWindowParam::Week => Window::Week,
            MeasureWindowParam::Month => Window::Month,
        }
    }
}

/// [`WindowParams`] with `1h`.
#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
pub(crate) struct MeasureWindowParams {
    #[serde(default)]
    #[param(inline)]
    pub(crate) window: MeasureWindowParam,
}

/// [`MeasureWindowParams`], moved `offset` back: the previous window is `offset=` its length.
#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
pub(crate) struct MeasureActivityParams {
    #[serde(default)]
    #[param(inline)]
    pub(crate) window: MeasureWindowParam,
    #[param(inline)]
    pub(crate) offset: Option<MeasureWindowParam>,
}

/// The step of a series.
#[derive(Debug, Deserialize, Clone, Copy, ToSchema)]
pub(crate) enum IntervalParam {
    #[serde(rename = "5m")]
    FiveMinutes,
    #[serde(rename = "1h")]
    Hour,
    #[serde(rename = "1d")]
    Day,
}

impl From<IntervalParam> for SeriesInterval {
    fn from(value: IntervalParam) -> Self {
        match value {
            IntervalParam::FiveMinutes => SeriesInterval::FiveMinutes,
            IntervalParam::Hour => SeriesInterval::Hour,
            IntervalParam::Day => SeriesInterval::Day,
        }
    }
}

/// A series over `window` at `interval`: `5m` over `1h` only, `1h` or `1d` over the others.
#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
pub(crate) struct SeriesParams {
    #[serde(default)]
    #[param(inline)]
    pub(crate) window: MeasureWindowParam,
    #[param(inline)]
    pub(crate) interval: IntervalParam,
}

impl SeriesParams {
    /// The window and its step, or 400 for a step that does not cut it.
    pub(crate) fn parse(self) -> Result<(Window, SeriesInterval), ApiError> {
        let window: Window = self.window.into();
        let interval: SeriesInterval = self.interval.into();
        if interval.fits(window) {
            Ok((window, interval))
        } else {
            Err(ApiError::BadRequest(
                "`interval=5m` cuts `window=1h` only, and `window=1h` is cut at `5m` only"
                    .to_string(),
            ))
        }
    }
}

/// The step of a series of readings: one per hour measured.
#[derive(Debug, Deserialize, Clone, Copy, ToSchema)]
pub(crate) enum ReadingIntervalParam {
    #[serde(rename = "1h")]
    Hour,
}

/// The readings over `window`, one per `interval`.
#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReadingSeriesParams {
    #[serde(default)]
    #[param(inline)]
    pub(crate) window: WindowParam,
    #[param(inline)]
    pub(crate) interval: ReadingIntervalParam,
}
