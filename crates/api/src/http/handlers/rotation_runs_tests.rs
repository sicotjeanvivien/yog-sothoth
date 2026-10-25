use axum::http::StatusCode;

use crate::testing::{get, pk};

/// The control: each route of a protocol's rotation runs reaches its handler.
#[tokio::test]
async fn the_rotation_run_routes_are_mounted() {
    for uri in [
        "/api/protocols/meteora_damm_v2/rotation-runs",
        "/api/protocols/meteora_damm_v2/rotation-runs?window=30d&sort=at&limit=200",
        "/api/protocols/meteora_damm_v2/rotation-runs/totals?window=7d",
        "/api/protocols/meteora_damm_v2/rotation-runs/hourly",
        "/api/protocols/meteora_damm_v2/rotation-runs/daily?window=30d",
    ] {
        let (status, _, body) = get(uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
    }
}

/// Mutation: the order or the protocol read as a free string, or a cursor decoded without its
/// list — an unknown order answered, or a page of another list followed.
#[tokio::test]
async fn an_unknown_order_protocol_or_a_cursor_of_another_list_is_refused() {
    use yog_core::{
        Cursor,
        domain::{Protocol, RotationRunCursor, RotationRunOrder, RotationRunScope, Window},
    };
    let encode = |order, window| {
        crate::http::cursor::encode_cursor(&Cursor::RotationRun(Box::new(RotationRunCursor {
            scope: RotationRunScope::Protocol(Protocol::MeteoraDammV2),
            window,
            as_of: crate::testing::ts(2_000),
            order,
            volume_usd: rust_decimal::Decimal::ONE,
            started_at: crate::testing::ts(1_000),
            pool_address: pk(1),
        })))
        .unwrap()
    };
    let by_volume = encode(RotationRunOrder::Volume, Window::Day);
    for uri in [
        "/api/protocols/meteora_damm_v2/rotation-runs?sort=amount".to_owned(),
        "/api/protocols/meteora/rotation-runs".to_owned(),
        "/api/protocols/meteora/rotation-runs/totals".to_owned(),
        format!("/api/protocols/meteora_damm_v2/rotation-runs?sort=at&cursor={by_volume}"),
        format!("/api/protocols/meteora_damm_v2/rotation-runs?window=7d&cursor={by_volume}"),
        format!("/api/pools/{}/rotation-runs?cursor={by_volume}", pk(1)),
    ] {
        let (status, content_type, detail) = get(&uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {detail}");
        assert_eq!(content_type, "application/problem+json", "{uri}");
    }
}

/// Mutation: the pool of the path not looked up — an unknown pool answered empty.
#[tokio::test]
async fn an_unknown_pool_has_no_rotation_runs() {
    let (status, content_type, _) = get(&format!("/api/pools/{}/rotation-runs", pk(42))).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(content_type, "application/problem+json");
}
