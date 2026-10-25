use axum::http::StatusCode;
use yog_core::{
    Cursor,
    domain::{PoolCursor, PoolScope, TokenPriceCursor},
};

use crate::http::cursor::encode_cursor;
use crate::testing::{get, pk, ts};

/// The control: each business route reaches its handler.
#[tokio::test]
async fn the_business_routes_are_mounted() {
    for uri in [
        "/api/pools",
        "/api/pools?limit=200",
        "/api/protocols/meteora_damm_v2/pools",
        "/api/tokens",
    ] {
        let (status, _, body) = get(uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
    }
}

/// Mutation: a cursor decoded without its list — a page of every pool followed under one
/// protocol, another mint's prices, or prices as tokens; or a path read without parsing — a
/// malformed one served.
#[tokio::test]
async fn a_malformed_path_or_a_cursor_of_another_list_is_refused() {
    let all = encode_cursor(&Cursor::Pool(Box::new(PoolCursor {
        scope: PoolScope::All,
        first_seen_at: ts(1_000),
        pool_address: pk(1),
    })))
    .unwrap();
    let other_mint = encode_cursor(&Cursor::TokenPrice(Box::new(TokenPriceCursor {
        mint: pk(3),
        from: ts(1_000),
        fetched_at: ts(2_000),
    })))
    .unwrap();
    for uri in [
        "/api/protocols/meteora/pools".to_owned(),
        "/api/protocols/meteora".to_owned(),
        "/api/pools/not-a-key".to_owned(),
        "/api/tokens/not-a-key/pools".to_owned(),
        "/api/tokens/not-a-key/prices".to_owned(),
        "/api/pools?limit=0".to_owned(),
        "/api/tokens?limit=201".to_owned(),
        format!("/api/tokens/{}/prices?window=1h", pk(2)),
        format!("/api/protocols/meteora_damm_v2/pools?cursor={all}"),
        format!("/api/tokens/{}/prices?cursor={other_mint}", pk(2)),
        format!("/api/tokens?cursor={other_mint}"),
    ] {
        let (status, content_type, _) = get(&uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(content_type, "application/problem+json", "{uri}");
    }
}

/// Mutation: the pool or the token not looked up — an unknown one answered empty.
#[tokio::test]
async fn an_unknown_pool_or_token_is_not_found() {
    for uri in [
        format!("/api/pools/{}", pk(42)),
        format!("/api/tokens/{}/pools", pk(42)),
        format!("/api/tokens/{}/prices", pk(42)),
    ] {
        let (status, content_type, _) = get(&uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
        assert_eq!(content_type, "application/problem+json", "{uri}");
    }
}
