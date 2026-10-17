//! Request DTOs: `XxxRequest::parse(...)` runs every validation of one endpoint once, so a
//! value cannot exist invalid. The helpers live in [`http::query`] and [`http::cursor`].
//!
//! [`http::query`]: crate::http::query
//! [`http::cursor`]: crate::http::cursor

mod get_token;
mod list_signals;

pub(crate) use get_token::GetTokenRequest;
pub(crate) use list_signals::ListSignalsRequest;
