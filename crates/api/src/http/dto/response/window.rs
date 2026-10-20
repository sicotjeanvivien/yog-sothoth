//! The envelope of a measure over a window: the window, the span it covers,
//! and the rows. ⚠️ A total whose span starts before `coveredSince` has no row.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{Span, Window};

use crate::application::Measured;

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
pub(in crate::http::dto::response) enum WindowResponse {
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
            Window::Day => Self::Day,
            Window::Week => Self::Week,
            Window::Month => Self::Month,
        }
    }
}

/// `from..asOf`, each `null` before the first hour is frozen.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowedResponse<T> {
    window: WindowResponse,
    from: Option<DateTime<Utc>>,
    /// The close of the last frozen hour: the instant the rows reflect.
    as_of: Option<DateTime<Utc>>,
    /// The first hour the facts hold.
    covered_since: Option<DateTime<Utc>>,
    items: Vec<T>,
}

impl<T> WindowedResponse<T> {
    pub(crate) fn new<U>(window: Window, measured: Measured<Vec<U>>) -> Self
    where
        T: From<U>,
    {
        let span: Option<Span> = measured.span;
        Self {
            window: window.into(),
            from: span.map(|s| s.from),
            as_of: span.map(|s| s.as_of),
            covered_since: span.map(|s| s.covered_since),
            items: measured.items.into_iter().map(T::from).collect(),
        }
    }
}
