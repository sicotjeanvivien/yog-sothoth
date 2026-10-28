use axum::{extract::Query, http::Uri};
use yog_core::domain::{LiquidityMoveKind, LiquidityMoveScope, Protocol};

use super::{LiquidityMovesParams, ListLiquidityMovesRequest};

fn kind_of(query: &str) -> Option<LiquidityMoveKind> {
    let uri: Uri = format!("/api/protocols/meteora_damm_v2/liquidity-moves{query}")
        .parse()
        .unwrap();
    let Query(params) = Query::<LiquidityMovesParams>::try_from_uri(&uri).unwrap();
    let scope = LiquidityMoveScope::Protocol(Protocol::MeteoraDammV2);
    ListLiquidityMovesRequest::parse(params, scope)
        .unwrap()
        .list
        .kind
}

/// Mutation: the kind of the query dropped, or mapped to another — the five largest moves of
/// any kind listed as the pools' first deposits.
#[test]
fn each_kind_of_the_query_reaches_the_read() {
    assert_eq!(kind_of(""), None);
    assert_eq!(kind_of("?kind=create"), Some(LiquidityMoveKind::Create));
    assert_eq!(kind_of("?kind=add"), Some(LiquidityMoveKind::Add));
    assert_eq!(kind_of("?kind=remove"), Some(LiquidityMoveKind::Remove));
}
