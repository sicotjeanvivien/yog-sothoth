use axum::http::StatusCode;

use crate::testing::{get, pk};

/// The control: a protocol's moves, their window, order and limit reach the handler.
#[tokio::test]
async fn the_protocol_moves_take_a_protocol_of_the_domain() {
    for uri in [
        "/api/protocols/meteora_damm_v2/liquidity-moves",
        "/api/protocols/meteora_damm_v2/liquidity-moves?window=7d&sort=at&limit=5",
    ] {
        let (status, _, body) = get(uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        assert!(body.contains("\"nextCursor\":null"), "{uri}: {body}");
    }
}

/// Mutation: the protocol, the order or the kind read as a free string, or a cursor decoded
/// without its scope — an unknown one answered empty, or a page of a pool followed elsewhere.
#[tokio::test]
async fn an_unknown_protocol_order_or_a_cursor_of_another_list_is_refused() {
    use yog_core::{
        Cursor,
        domain::{
            LiquidityMoveCursor, LiquidityMoveList, LiquidityMoveOrder, LiquidityMoveScope, Window,
        },
    };
    let pool_cursor =
        crate::http::cursor::encode_cursor(&Cursor::LiquidityMove(Box::new(LiquidityMoveCursor {
            list: LiquidityMoveList::every_kind(LiquidityMoveScope::Pool(pk(1))),
            window: Window::Day,
            as_of: chrono::Utc::now(),
            order: LiquidityMoveOrder::Amount,
            amount_usd: rust_decimal::Decimal::ONE,
            at: chrono::Utc::now(),
            signature: "sig".to_owned(),
            event_index: 0,
        })))
        .unwrap();

    for uri in [
        "/api/protocols/meteora/liquidity-moves".to_owned(),
        "/api/protocols/meteora_damm_v2/liquidity-moves?limit=0".to_owned(),
        "/api/protocols/meteora_damm_v2/liquidity-moves?limit=201".to_owned(),
        "/api/protocols/meteora_damm_v2/liquidity-moves?window=1h".to_owned(),
        "/api/protocols/meteora_damm_v2/liquidity-moves?sort=volume".to_owned(),
        "/api/protocols/meteora_damm_v2/liquidity-moves?kind=burn".to_owned(),
        format!("/api/protocols/meteora_damm_v2/liquidity-moves?cursor={pool_cursor}"),
    ] {
        let (status, content_type, body) = get(&uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {body}");
        assert_eq!(content_type, "application/problem+json", "{uri}");
    }
}
