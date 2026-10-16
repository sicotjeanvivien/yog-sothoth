//! DB-free tests of `TryFrom<PoolLiquidityFlowRow> for PoolLiquidityFlow`.

use rust_decimal::Decimal;
use yog_core::{RepositoryError, domain::PoolLiquidityFlow};

use super::PoolLiquidityFlowRow;

const VALID_POOL: &str = "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1";

fn valid_row() -> PoolLiquidityFlowRow {
    PoolLiquidityFlowRow {
        pool_address: VALID_POOL.into(),
        // Mutation: two fields swapped in the TryFrom.
        added_usd: Some("1234.5".parse().unwrap()),
        removed_usd: Some("678.9".parse().unwrap()),
        tvl_usd: Some("42000.25".parse().unwrap()),
    }
}

#[test]
fn try_from_valid_row_maps_each_field() {
    let flow = PoolLiquidityFlow::try_from(valid_row()).expect("valid row should convert");

    assert_eq!(flow.pool_address.to_string(), VALID_POOL);
    assert_eq!(flow.added_usd, Some(Decimal::new(12_345, 1)));
    assert_eq!(flow.removed_usd, Some(Decimal::new(6_789, 1)));
    assert_eq!(flow.tvl_usd, Some(Decimal::new(4_200_025, 2)));
}

#[test]
fn try_from_null_tvl_stays_none() {
    let row = PoolLiquidityFlowRow {
        tvl_usd: None,
        ..valid_row()
    };
    let flow = PoolLiquidityFlow::try_from(row).expect("valid row should convert");
    assert_eq!(flow.tvl_usd, None);
}

#[test]
fn try_from_null_flows_map_to_none_rather_than_zero() {
    // Mutation: a NULL (window not entirely valuable) mapped to `Decimal::ZERO`,
    // which reads as a healthy pool.
    let row = PoolLiquidityFlowRow {
        added_usd: None,
        removed_usd: None,
        ..valid_row()
    };
    let flow = PoolLiquidityFlow::try_from(row).expect("a NULL flow is valid, not an error");

    assert_eq!(flow.added_usd, None);
    assert_eq!(flow.removed_usd, None);
    assert_eq!(
        flow.tvl_usd,
        Some(Decimal::new(4_200_025, 2)),
        "an unvaluable window says nothing about the current TVL — the two are \
         independent, and conflating them would hide one behind the other"
    );
}

#[test]
fn try_from_invalid_pool_address_returns_integrity() {
    let row = PoolLiquidityFlowRow {
        pool_address: "not-a-pubkey".into(),
        ..valid_row()
    };
    let err = PoolLiquidityFlow::try_from(row).expect_err("bad pubkey should fail");
    assert!(matches!(err, RepositoryError::Integrity(_)));
}
