use axum::http::StatusCode;

use crate::testing::{get, pk};

/// The control: each produced route reaches its handler on a known protocol.
#[tokio::test]
async fn the_produced_routes_are_mounted() {
    for uri in [
        "/api/swaps/totals?window=7d&offset=7d",
        "/api/swaps/hourly",
        "/api/swaps/daily?window=30d",
        "/api/swaps/by-token?window=7d&limit=10",
        "/api/protocols/meteora_damm_v2/swaps/totals",
        "/api/protocols/meteora_damm_v2/swaps/hourly",
        "/api/protocols/meteora_damm_v2/swaps/daily",
        "/api/pool-creations/totals",
        "/api/pool-creations/hourly",
        "/api/pool-creations/daily",
        "/api/protocols/meteora_damm_v2/pool-creations/totals",
        "/api/protocols/meteora_damm_v2/pool-creations/hourly",
        "/api/protocols/meteora_damm_v2/pool-creations/daily",
        "/api/protocols/meteora_damm_v2/liquidity-moves/totals",
        "/api/protocols/meteora_damm_v2/liquidity-moves/hourly",
        "/api/protocols/meteora_damm_v2/liquidity-moves/daily",
        "/api/tvl",
        "/api/tvl/hourly?window=7d",
        "/api/protocols/meteora_damm_v2/tvl",
        "/api/protocols/meteora_damm_v2/tvl/hourly",
        "/api/pool-discoveries/totals?window=30d",
        "/api/protocols/meteora_damm_v2/pool-discoveries/totals",
        "/api/protocols/meteora_damm_v2/signals",
    ] {
        let (status, _, body) = get(uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
    }
}

/// Mutation: the protocol of the path read as a free string, or a window outside its list
/// taken — an unknown one answered empty.
#[tokio::test]
async fn an_unknown_protocol_or_window_is_refused_in_problem_details() {
    for uri in [
        "/api/protocols/meteora/swaps/totals",
        "/api/protocols/meteora/swaps/daily",
        "/api/protocols/meteora/pool-creations/totals",
        "/api/protocols/meteora/liquidity-moves/hourly",
        "/api/protocols/meteora/tvl",
        "/api/protocols/meteora/tvl/hourly",
        "/api/protocols/meteora/pool-discoveries/totals",
        "/api/protocols/meteora/signals",
        "/api/swaps/totals?window=1h",
        "/api/tvl/hourly?window=90d",
    ] {
        let (status, content_type, _) = get(uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(content_type, "application/problem+json", "{uri}");
    }
}

/// Mutation: the pool of the path not looked up — an unknown pool answered empty; or a
/// malformed one read as unknown.
#[tokio::test]
async fn a_pool_is_refused_malformed_and_not_found_unknown() {
    let unknown = pk(42);
    for (uri, expected) in [
        (
            "/api/pools/not-a-key/swaps/totals".to_owned(),
            StatusCode::BAD_REQUEST,
        ),
        (
            format!("/api/pools/{unknown}/swaps/totals"),
            StatusCode::NOT_FOUND,
        ),
        (
            format!("/api/pools/{unknown}/swaps/hourly"),
            StatusCode::NOT_FOUND,
        ),
        (
            format!("/api/pools/{unknown}/swaps/daily"),
            StatusCode::NOT_FOUND,
        ),
        (
            format!("/api/pools/{unknown}/liquidity-moves"),
            StatusCode::NOT_FOUND,
        ),
        (
            format!("/api/pools/{unknown}/signals"),
            StatusCode::NOT_FOUND,
        ),
    ] {
        let (status, content_type, _) = get(&uri).await;
        assert_eq!(status, expected, "{uri}");
        assert_eq!(content_type, "application/problem+json", "{uri}");
    }
}
