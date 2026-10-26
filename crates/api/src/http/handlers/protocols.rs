//! `/api/protocols`: the catalogue, read live, and one protocol's entry of it.

use axum::{
    Json,
    extract::{Path, Query, State},
};

use crate::http::{
    AppState,
    dto::{
        PageResponse, ProtocolCatalogResponse,
        request::{ListProtocolsRequest, NoQuery, ProtocolsParams, parse_protocol},
    },
    error::ApiError,
};

#[utoipa::path(
    get,
    path = "/api/protocols",
    params(ProtocolsParams),
    responses((
        status = 200,
        description = "A page of every protocol of the domain, observed or not.",
        body = PageResponse<ProtocolCatalogResponse>
    ))
)]
pub(crate) async fn list_protocols(
    State(state): State<AppState>,
    Query(params): Query<ProtocolsParams>,
) -> Result<Json<PageResponse<ProtocolCatalogResponse>>, ApiError> {
    let request = ListProtocolsRequest::parse(params)?;
    let page = state
        .protocol_catalog_service
        .list_page(request.after, request.limit)
        .await?;
    Ok(Json(PageResponse::from_page(page)?))
}

#[utoipa::path(
    get,
    path = "/api/protocols/{protocol}",
    responses((
        status = 200,
        description = "The protocol's entry of the catalogue, observed or not.",
        body = ProtocolCatalogResponse
    ))
)]
pub(crate) async fn get_protocol(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    _query: Query<NoQuery>,
) -> Result<Json<ProtocolCatalogResponse>, ApiError> {
    let protocol = parse_protocol(&protocol)?;
    let entry = state
        .protocol_catalog_service
        .get_protocol(protocol)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("protocol not in the catalogue: {protocol}")))?;
    Ok(Json(entry.into()))
}

#[cfg(test)]
#[path = "protocols_tests.rs"]
mod tests;
