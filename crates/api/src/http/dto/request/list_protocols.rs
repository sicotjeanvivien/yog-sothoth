//! Request DTO for `GET /api/protocols`.

use serde::Deserialize;
use utoipa::IntoParams;
use yog_core::domain::Protocol;

use crate::http::{cursor::decode_protocol_cursor, error::ApiError};

/// Forward-only cursor pagination over the catalogue.
#[derive(Debug, Deserialize, IntoParams)]
pub(crate) struct ProtocolsParams {
    #[param(min_length = 1)]
    cursor: Option<String>,
    #[serde(default = "super::default_limit")]
    #[param(value_type = u8, minimum = 1, maximum = 200)]
    limit: i64,
}

#[derive(Debug)]
pub(crate) struct ListProtocolsRequest {
    /// The last protocol of the page before.
    pub(crate) after: Option<Protocol>,
    pub(crate) limit: i64,
}

impl ListProtocolsRequest {
    pub(crate) fn parse(params: ProtocolsParams) -> Result<Self, ApiError> {
        super::validate_limit(params.limit)?;
        let after = params
            .cursor
            .as_deref()
            .map(|raw| super::non_empty("cursor", raw).and_then(decode_protocol_cursor))
            .transpose()?;
        Ok(Self {
            after,
            limit: params.limit,
        })
    }
}
