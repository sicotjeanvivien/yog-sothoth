//! The `window` and `offset` of a measure: the closed list of `Window`.

use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use yog_core::domain::Window;

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

/// [`WindowParams`], moved `offset` back: the previous window is `offset=` its length.
#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
pub(crate) struct ActivityParams {
    #[serde(default)]
    #[param(inline)]
    pub(crate) window: WindowParam,
    #[param(inline)]
    pub(crate) offset: Option<WindowParam>,
}
