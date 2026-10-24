use axum::http::StatusCode;

use crate::testing::get;

/// Mutation: `window` read as a free string, an unknown one answered as the default.
#[tokio::test]
async fn a_window_outside_its_list_is_refused_in_problem_details() {
    for uri in [
        "/api/protocols/activity?window=1h",
        "/api/protocols/activity?offset=1h",
        "/api/protocols/activity/daily?window=90d",
        "/api/protocols/tvl/hourly?window=24",
        "/api/tokens/activity?window=1h",
        "/api/tokens/activity?sort=pools",
    ] {
        let (status, content_type, _) = get(uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(content_type, "application/problem+json", "{uri}");
    }
}

/// The control: each window of the list, and none, reaches its handler.
#[tokio::test]
async fn the_windows_of_the_list_are_taken() {
    for uri in [
        "/api/protocols/activity",
        "/api/protocols/activity?window=7d&offset=7d",
        "/api/protocols/activity/hourly?window=24h",
        "/api/protocols/activity/daily?window=30d",
        "/api/protocols/tvl",
        "/api/protocols/tvl/hourly?window=7d",
    ] {
        let (status, _, _) = get(uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
    }
}

/// Mutation: the route left out of `api_routes` — `/api/tokens/{mint}` then refuses
/// `activity` as a mint.
#[tokio::test]
async fn the_tokens_activity_is_not_read_as_a_mint() {
    let (status, _, body) = get("/api/tokens/activity?window=7d&limit=10").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("\"window\":\"7d\""), "{body}");
}

/// Mutation: the protocol read as a free string, an unknown one answered empty.
#[tokio::test]
async fn the_liquidity_moves_refuse_an_unknown_protocol_or_limit() {
    for uri in [
        "/api/protocols/meteora/liquidity-moves",
        "/api/protocols/meteora_damm_v2/liquidity-moves?limit=0",
        "/api/protocols/meteora_damm_v2/liquidity-moves?limit=201",
        "/api/protocols/meteora_damm_v2/liquidity-moves?window=1h",
    ] {
        let (status, content_type, _) = get(uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(content_type, "application/problem+json", "{uri}");
    }
}

/// The control: a known protocol, its window and its limit reach the handler.
#[tokio::test]
async fn the_liquidity_moves_take_a_protocol_of_the_domain() {
    let (status, _, body) =
        get("/api/protocols/meteora_damm_v2/liquidity-moves?window=7d&limit=5").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("\"window\":\"7d\""), "{body}");
}
