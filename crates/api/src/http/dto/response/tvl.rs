//! The resource `tvl`: a reading at an instant, its change over a window, the part only DAMM v2
//! has, and its cut by token.

mod by_token;
mod by_token_page;
mod change;
mod meteora_damm_v2;
mod reading;

pub(crate) use by_token_page::TokenTvlPageResponse;
pub(crate) use change::ProtocolTvlChangeResponse;
pub(crate) use reading::ProtocolTvlResponse;
