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

/// The control: each on-chain event route reaches its handler.
#[tokio::test]
async fn the_event_routes_are_mounted() {
    for uri in [
        "/api/swaps",
        "/api/swaps?window=24h&limit=200",
        "/api/protocols/meteora_damm_v2/swaps",
        "/api/liquidity-changes?window=30d",
        "/api/protocols/meteora_damm_v2/liquidity-changes",
        "/api/pool-creations?window=7d",
        "/api/protocols/meteora_damm_v2/pool-creations",
    ] {
        let (status, _, body) = get(uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
    }
}

/// Mutation: the widest window not checked — every pool's swaps read over 7 days, a pool's over
/// 30 days its swaps are not kept, or a forged cursor's start past the day; or a cursor decoded
/// without its collection or scope — a page of another list followed.
#[tokio::test]
async fn a_window_past_a_lists_widest_or_a_cursor_of_another_list_is_refused() {
    use yog_core::{
        Cursor,
        domain::{EventCursor, EventScope},
    };
    let of = |scope| EventCursor {
        scope,
        from: crate::testing::ts(1_000),
        at: crate::testing::ts(2_000),
        signature: solana_signature::Signature::from([1; 64]),
        event_index: 0,
    };
    let encode = |cursor: Cursor| crate::http::cursor::encode_cursor(&cursor).unwrap();
    let recent = |scope| EventCursor {
        from: chrono::Utc::now() - chrono::Duration::hours(2),
        at: chrono::Utc::now() - chrono::Duration::hours(1),
        ..of(scope)
    };
    let liquidity = encode(Cursor::LiquidityChangeEvent(Box::new(recent(
        EventScope::All,
    ))));
    let all_swaps = encode(Cursor::SwapEvent(Box::new(recent(EventScope::All))));
    let forged = encode(Cursor::SwapEvent(Box::new(EventCursor {
        from: chrono::Utc::now() - chrono::Duration::days(8),
        ..recent(EventScope::All)
    })));
    let backwards = encode(Cursor::SwapEvent(Box::new(EventCursor {
        at: chrono::Utc::now() - chrono::Duration::hours(3),
        ..recent(EventScope::All)
    })));
    for uri in [
        "/api/swaps?window=7d".to_owned(),
        "/api/protocols/meteora_damm_v2/swaps?window=30d".to_owned(),
        format!("/api/pools/{}/swaps?window=30d", pk(1)),
        format!("/api/swaps?cursor={forged}"),
        format!("/api/swaps?cursor={backwards}"),
        "/api/protocols/meteora/liquidity-changes".to_owned(),
        format!("/api/swaps?cursor={liquidity}"),
        format!("/api/protocols/meteora_damm_v2/swaps?cursor={all_swaps}"),
        format!("/api/pools/{}/swaps?cursor={all_swaps}", pk(1)),
    ] {
        let (status, content_type, detail) = get(&uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {detail}");
        assert_eq!(content_type, "application/problem+json", "{uri}");
    }
}

/// Mutation: the pool of the path not looked up — an unknown pool answered empty, over a
/// window its list serves.
#[tokio::test]
async fn an_unknown_pool_has_no_events() {
    for uri in [
        format!("/api/pools/{}/swaps?window=7d", pk(42)),
        format!("/api/pools/{}/liquidity-changes?window=30d", pk(42)),
    ] {
        let (status, content_type, _) = get(&uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
        assert_eq!(content_type, "application/problem+json", "{uri}");
    }
}

/// Mutation: no margin past the widest window — a next page read moments after its first,
/// whose window then started a little more than a day ago, refused.
#[tokio::test]
async fn a_next_page_of_the_day_is_served() {
    use yog_core::{
        Cursor,
        domain::{EventCursor, EventScope},
    };
    let next = crate::http::cursor::encode_cursor(&Cursor::SwapEvent(Box::new(EventCursor {
        scope: EventScope::All,
        from: chrono::Utc::now() - chrono::Duration::hours(24) - chrono::Duration::minutes(1),
        at: chrono::Utc::now() - chrono::Duration::hours(1),
        signature: solana_signature::Signature::from([1; 64]),
        event_index: 0,
    })))
    .unwrap();

    let (status, _, detail) = get(&format!("/api/swaps?cursor={next}")).await;

    assert_eq!(status, StatusCode::OK, "{detail}");
}
