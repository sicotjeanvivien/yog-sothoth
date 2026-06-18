use super::*;

fn sig(seed: u8) -> Signature {
    Signature::from([seed; 64])
}

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

#[test]
fn last_event_kind_roundtrip() {
    for kind in [
        LastEventKind::Swap,
        LastEventKind::LiquidityAdd,
        LastEventKind::LiquidityRemove,
    ] {
        assert_eq!(LastEventKind::from_wire(kind.as_str()), Some(kind));
    }
}

#[test]
fn last_event_kind_rejects_unknown() {
    assert_eq!(LastEventKind::from_wire("unknown"), None);
    assert_eq!(LastEventKind::from_wire(""), None);
}

#[test]
fn last_event_kind_from_liquidity_event_kind() {
    assert_eq!(
        LastEventKind::from(MeteoraDammV2LiquidityEventKind::Add),
        LastEventKind::LiquidityAdd
    );
    assert_eq!(
        LastEventKind::from(MeteoraDammV2LiquidityEventKind::Remove),
        LastEventKind::LiquidityRemove
    );
}

#[test]
fn from_swap_marks_kind_as_swap_and_sets_only_sqrt_price() {
    let now = Utc::now();
    let upsert = PoolCurrentStateUpsert::from_swap(
        pk(1),
        Protocol::MeteoraDammV2,
        now,
        sig(1),
        100,
        200,
        9_999,
    );
    assert_eq!(upsert.event_kind, LastEventKind::Swap);
    assert_eq!(upsert.sqrt_price, Some(9_999));
    assert_eq!(upsert.liquidity, None);
}

#[test]
fn from_liquidity_maps_kind_through_domain_enum() {
    let now = Utc::now();
    let add = PoolCurrentStateUpsert::from_liquidity(
        pk(1),
        Protocol::MeteoraDammV2,
        now,
        sig(1),
        MeteoraDammV2LiquidityEventKind::Add,
        100,
        200,
        42,
    );
    let remove = PoolCurrentStateUpsert::from_liquidity(
        pk(1),
        Protocol::MeteoraDammV2,
        now,
        sig(1),
        MeteoraDammV2LiquidityEventKind::Remove,
        100,
        200,
        42,
    );
    assert_eq!(add.event_kind, LastEventKind::LiquidityAdd);
    assert_eq!(remove.event_kind, LastEventKind::LiquidityRemove);
    assert_eq!(add.sqrt_price, None);
    assert_eq!(add.liquidity, Some(42));
}
