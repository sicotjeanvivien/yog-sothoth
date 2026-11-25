use axum::http::StatusCode;

use crate::testing::get;

/// The control: both new routes reach their handlers, and answer an empty page before a reading.
#[tokio::test]
async fn the_locked_liquidity_routes_are_mounted() {
    for uri in [
        "/api/protocols/meteora_damm_v2/tvl/by-token",
        "/api/protocols/meteora_damm_v2/tvl/by-token?sort=locked&limit=200",
        "/api/protocols/meteora_damm_v2/vesting-unlocks",
        "/api/protocols/meteora_damm_v2/vesting-unlocks?window=30d&sort=amount&limit=5",
    ] {
        let (status, _, body) = get(uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        assert!(body.contains("\"items\":[]"), "{uri}: {body}");
    }
}

/// Mutation: an order, a window or a protocol read as a free string — the TVL's `1h`, or a sort of
/// another list, answered.
#[tokio::test]
async fn an_unknown_order_window_or_protocol_is_refused() {
    for uri in [
        "/api/protocols/meteora_damm_v2/tvl/by-token?sort=volume",
        "/api/protocols/meteora_damm_v2/tvl/by-token?window=24h",
        "/api/protocols/meteora/tvl/by-token",
        "/api/protocols/meteora_damm_v2/vesting-unlocks?window=1h",
        "/api/protocols/meteora_damm_v2/vesting-unlocks?sort=tvl",
        "/api/protocols/meteora_damm_v2/vesting-unlocks?offset=1",
    ] {
        let (status, _, body) = get(uri).await;
        assert!(
            status == StatusCode::BAD_REQUEST || status == StatusCode::NOT_FOUND,
            "{uri}: {status} {body}"
        );
    }
}

/// Mutation: a cursor decoded without its reading, its window or its order — a page of another
/// list followed, or one past its window.
#[tokio::test]
async fn a_cursor_of_another_list_or_past_its_window_is_refused() {
    use yog_core::{
        Cursor,
        domain::{Protocol, VestingUnlockCursor, VestingUnlockOrder, Window},
    };
    let now = chrono::Utc::now();
    let encode = |order, window, ahead| {
        crate::http::cursor::encode_cursor(&Cursor::VestingUnlock(Box::new(VestingUnlockCursor {
            protocol: Protocol::MeteoraDammV2,
            as_of: now,
            window,
            order,
            amount_usd: None,
            unlocks_at: now + ahead,
            pool_address: crate::testing::pk(1),
            position: crate::testing::pk(2),
        })))
        .unwrap()
    };
    let week = encode(
        VestingUnlockOrder::Time,
        Window::Week,
        chrono::Duration::days(1),
    );
    let past = encode(
        VestingUnlockOrder::Time,
        Window::Day,
        chrono::Duration::days(2),
    );
    for uri in [
        format!(
            "/api/protocols/meteora_damm_v2/vesting-unlocks?window=7d&sort=amount&cursor={week}"
        ),
        format!("/api/protocols/meteora_damm_v2/vesting-unlocks?window=24h&cursor={week}"),
        format!("/api/protocols/meteora_damm_v2/vesting-unlocks?window=24h&cursor={past}"),
        // Valid for its list, but no reading holds it: expired.
        format!("/api/protocols/meteora_damm_v2/vesting-unlocks?window=7d&cursor={week}"),
    ] {
        let (status, _, body) = get(&uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {body}");
    }
}
