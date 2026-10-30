use axum::http::StatusCode;

use crate::testing::{get, pk};

/// The control: each produced route reaches its handler on a known protocol.
#[tokio::test]
async fn the_produced_routes_are_mounted() {
    for uri in [
        "/api/swaps/totals?window=7d&offset=7d",
        "/api/swaps/series?interval=1h",
        "/api/swaps/series?window=30d&interval=1d",
        "/api/swaps/by-token?window=7d&limit=10",
        "/api/protocols/meteora_damm_v2/swaps/totals",
        "/api/protocols/meteora_damm_v2/swaps/series?interval=1h",
        "/api/protocols/meteora_damm_v2/swaps/series?interval=1d",
        "/api/pool-creations/totals",
        "/api/pool-creations/series?interval=1h",
        "/api/pool-creations/series?interval=1d",
        "/api/protocols/meteora_damm_v2/pool-creations/totals",
        "/api/protocols/meteora_damm_v2/pool-creations/series?interval=1h",
        "/api/protocols/meteora_damm_v2/pool-creations/series?interval=1d",
        "/api/protocols/meteora_damm_v2/liquidity-moves/totals",
        "/api/protocols/meteora_damm_v2/liquidity-moves/series?interval=1h",
        "/api/protocols/meteora_damm_v2/liquidity-moves/series?window=30d&interval=1d",
        "/api/protocols/meteora_damm_v2/position-changes/totals?window=7d",
        "/api/tvl",
        "/api/tvl/series?window=7d&interval=1h",
        "/api/protocols/meteora_damm_v2/tvl",
        "/api/protocols/meteora_damm_v2/tvl/series?interval=1h",
        "/api/tvl/totals?window=7d",
        "/api/protocols/meteora_damm_v2/tvl/totals",
        "/api/pool-discoveries/totals?window=30d",
        "/api/protocols/meteora_damm_v2/pool-discoveries/totals",
        "/api/protocols/meteora_damm_v2/signals",
    ] {
        let (status, _, body) = get(uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
    }
}

/// Mutation: an `hourly` or `daily` route kept, or a series taking a step it does not serve — 5
/// minutes over the frozen hours, a day over the TVL's hourly readings, or no step.
#[tokio::test]
async fn a_series_is_cut_by_its_interval_only() {
    let pool = pk(1);
    for uri in [
        "/api/swaps/hourly".to_owned(),
        "/api/swaps/daily".to_owned(),
        "/api/protocols/meteora_damm_v2/swaps/hourly".to_owned(),
        "/api/protocols/meteora_damm_v2/swaps/daily".to_owned(),
        format!("/api/pools/{pool}/swaps/hourly"),
        format!("/api/pools/{pool}/swaps/daily"),
        "/api/pool-creations/hourly".to_owned(),
        "/api/pool-creations/daily".to_owned(),
        "/api/protocols/meteora_damm_v2/pool-creations/hourly".to_owned(),
        "/api/protocols/meteora_damm_v2/pool-creations/daily".to_owned(),
        "/api/protocols/meteora_damm_v2/liquidity-moves/hourly".to_owned(),
        "/api/protocols/meteora_damm_v2/liquidity-moves/daily".to_owned(),
        "/api/protocols/meteora_damm_v2/rotation-runs/hourly".to_owned(),
        "/api/protocols/meteora_damm_v2/rotation-runs/daily".to_owned(),
        "/api/tvl/hourly".to_owned(),
        "/api/protocols/meteora_damm_v2/tvl/hourly".to_owned(),
    ] {
        let (status, _, _) = get(&uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
    let pool_series = |query: &str| format!("/api/pools/{}/swaps/series?{query}", pk(1));
    for uri in [
        pool_series("interval=5m"),
        pool_series("window=1h&interval=1h"),
        "/api/protocols/meteora_damm_v2/liquidity-moves/series?interval=5m".to_owned(),
        "/api/protocols/meteora_damm_v2/rotation-runs/series?interval=5m".to_owned(),
        "/api/tvl/series?interval=1d".to_owned(),
        "/api/tvl/series".to_owned(),
        "/api/protocols/meteora_damm_v2/liquidity-moves/series".to_owned(),
    ] {
        let (status, content_type, _) = get(&uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(content_type, "application/problem+json", "{uri}");
    }
}

/// Mutation: a series refusing a step that does not fit its window in words of its own.
#[tokio::test]
async fn one_error_is_one_message_on_every_series() {
    let pool = pk(1);
    let mut details = Vec::new();
    for path in [
        "/api/swaps/series".to_owned(),
        "/api/protocols/meteora_damm_v2/swaps/series".to_owned(),
        format!("/api/pools/{pool}/swaps/series"),
        "/api/pool-creations/series".to_owned(),
        "/api/protocols/meteora_damm_v2/pool-creations/series".to_owned(),
        "/api/protocols/meteora_damm_v2/liquidity-moves/series".to_owned(),
        "/api/protocols/meteora_damm_v2/rotation-runs/series".to_owned(),
        "/api/protocols/meteora_damm_v2/position-changes/series".to_owned(),
        "/api/protocols/meteora_damm_v2/position-fee-claims/series".to_owned(),
        "/api/protocols/meteora_damm_v2/reward-claims/series".to_owned(),
    ] {
        let (status, _, detail) = get(&format!("{path}?window=24h&interval=5m")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        details.push(detail);
    }
    details.dedup();
    assert_eq!(details.len(), 1, "{details:?}");
}

/// Mutation: the protocol of the path read as a free string, or a window outside its list
/// taken — an unknown one answered empty.
#[tokio::test]
async fn an_unknown_protocol_or_window_is_refused_in_problem_details() {
    for uri in [
        "/api/protocols/meteora/swaps/totals",
        "/api/protocols/meteora/swaps/series?interval=1d",
        "/api/protocols/meteora/pool-creations/totals",
        "/api/protocols/meteora/liquidity-moves/series?interval=1h",
        "/api/protocols/meteora/position-changes/totals",
        "/api/protocols/meteora/tvl",
        "/api/protocols/meteora/tvl/series?interval=1h",
        "/api/protocols/meteora/tvl/totals",
        "/api/protocols/meteora/pool-discoveries/totals",
        "/api/protocols/meteora/signals",
        "/api/swaps/totals?window=90d",
        "/api/tvl/series?window=90d&interval=1h",
        "/api/tvl/totals?window=90d",
    ] {
        let (status, content_type, _) = get(uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(content_type, "application/problem+json", "{uri}");
    }
}

/// Mutation: `1h` taken where it is not served — the TVL, the pools discovered, a list of events —
/// or refused by the moves or the runs; or a series cut at a step that does not fit its window.
#[tokio::test]
async fn the_last_hour_and_its_step_are_refused_where_they_do_not_fit() {
    for uri in [
        "/api/tvl/totals?window=1h",
        "/api/protocols/meteora_damm_v2/pool-discoveries/totals?window=1h",
        "/api/protocols/meteora_damm_v2/swaps?window=1h",
        "/api/protocols/meteora_damm_v2/liquidity-moves/series?window=1h&interval=1h",
        "/api/swaps/series?window=24h&interval=5m",
        "/api/swaps/series?window=1h&interval=1h",
        "/api/swaps/series?window=24h",
        "/api/pool-creations/series?window=1h&interval=1d",
    ] {
        let (status, content_type, _) = get(uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(content_type, "application/problem+json", "{uri}");
    }
    for uri in [
        "/api/swaps/totals?window=1h&offset=1h",
        "/api/swaps/series?window=1h&interval=5m",
        "/api/swaps/series?window=7d&interval=1h",
        "/api/protocols/meteora_damm_v2/swaps/series?window=30d&interval=1d",
        "/api/pool-creations/totals?window=1h",
        "/api/pool-creations/series?window=1h&interval=5m",
        "/api/swaps/by-token?window=1h",
        "/api/protocols/meteora_damm_v2/rotation-runs/totals?window=1h",
        "/api/protocols/meteora_damm_v2/rotation-runs/series?window=1h&interval=5m",
        "/api/protocols/meteora_damm_v2/liquidity-moves/totals?window=1h",
        "/api/protocols/meteora_damm_v2/liquidity-moves/series?window=1h&interval=5m",
        "/api/protocols/meteora_damm_v2/liquidity-moves?window=1h",
        "/api/protocols/meteora_damm_v2/rotation-runs?window=1h",
        "/api/protocols/meteora_damm_v2/position-changes/totals?window=1h",
        "/api/protocols/meteora_damm_v2/position-changes/series?window=1h&interval=5m",
        "/api/protocols/meteora_damm_v2/position-fee-claims/totals?window=1h",
        "/api/protocols/meteora_damm_v2/position-fee-claims/series?window=1h&interval=5m",
        "/api/protocols/meteora_damm_v2/reward-claims/totals?window=1h",
        "/api/protocols/meteora_damm_v2/reward-claims/series?window=7d&interval=1h",
        "/api/protocols/meteora_damm_v2/position-changes/series?window=30d&interval=1d",
    ] {
        let (status, _, body) = get(uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
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
            format!("/api/pools/{unknown}/swaps/series?interval=1h"),
            StatusCode::NOT_FOUND,
        ),
        (
            format!("/api/pools/{unknown}/swaps/series?window=30d&interval=1d"),
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
        "/api/position-changes?window=30d",
        "/api/protocols/meteora_damm_v2/position-changes",
        "/api/position-fee-claims?window=30d",
        "/api/protocols/meteora_damm_v2/position-fee-claims",
        "/api/protocol-fee-claims?window=30d",
        "/api/protocols/meteora_damm_v2/protocol-fee-claims",
        "/api/reward-claims?window=30d",
        "/api/protocols/meteora_damm_v2/reward-claims",
        "/api/reward-changes?window=30d",
        "/api/protocols/meteora_damm_v2/reward-changes",
        "/api/pool-changes?window=30d",
        "/api/protocols/meteora_damm_v2/pool-changes",
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
    let fee_claims = encode(Cursor::PositionFeeClaimEvent(Box::new(recent(
        EventScope::All,
    ))));
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
        "/api/protocols/meteora/position-changes".to_owned(),
        "/api/protocols/meteora/reward-claims".to_owned(),
        format!("/api/position-changes?cursor={all_swaps}"),
        format!("/api/reward-claims?cursor={fee_claims}"),
        format!("/api/protocol-fee-claims?cursor={fee_claims}"),
        format!("/api/position-fee-claims?cursor={liquidity}"),
        "/api/protocols/meteora/reward-changes".to_owned(),
        "/api/protocols/meteora/pool-changes".to_owned(),
        format!("/api/reward-changes?cursor={fee_claims}"),
        format!("/api/pool-changes?cursor={all_swaps}"),
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
        format!("/api/pools/{}/position-changes?window=30d", pk(42)),
        format!("/api/pools/{}/position-fee-claims?window=30d", pk(42)),
        format!("/api/pools/{}/protocol-fee-claims?window=30d", pk(42)),
        format!("/api/pools/{}/reward-claims?window=30d", pk(42)),
        format!("/api/pools/{}/reward-changes?window=30d", pk(42)),
        format!("/api/pools/{}/pool-changes?window=30d", pk(42)),
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

/// Mutation: the advice to read under a pool given under a pool, where no list serves wider.
#[tokio::test]
async fn a_window_refused_under_a_pool_points_nowhere_else() {
    let (status, _, at_root) = get("/api/swaps?window=7d").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, under_pool) = get(&format!("/api/pools/{}/swaps?window=30d", pk(1))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    assert!(at_root.contains("read it under a pool"), "{at_root}");
    assert!(!under_pool.contains("under a pool"), "{under_pool}");
}
