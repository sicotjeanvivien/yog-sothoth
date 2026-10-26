use super::*;

const FEE_UPDATE_OPERATOR_SET: [&str; 4] = [
    // DCyrA6du…: the base fee set to 1.5 %.
    "01c0e1e400000000000000",
    // FPpQsmsc…: to 86 %.
    "01d88e4233000000000000",
    // 6MjqFccy… and 2y6PCqa4…: to 1 %.
    "0180969800000000000000",
    "0180969800000000000000",
];

fn bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).expect("hex"))
        .collect()
}

/// The dynamic fee most pools of the snapshot carry, `variable_fee_control` apart.
fn dynamic_fee(variable_fee_control: u32) -> MeteoraDammV2DynamicFee {
    MeteoraDammV2DynamicFee {
        bin_step: 1,
        bin_step_u128: 1_844_674_407_370_955,
        filter_period: 10,
        decay_period: 120,
        reduction_factor: 5000,
        max_volatility_accumulator: 14_460_000,
        variable_fee_control,
    }
}

fn time(
    decay: MeteoraDammV2FeeDecay,
    cliff_fee_numerator: u64,
    number_of_period: u16,
    period_frequency: u64,
    reduction_factor: u64,
) -> MeteoraDammV2BaseFee {
    MeteoraDammV2BaseFee::TimeScheduler {
        decay,
        cliff_fee_numerator,
        number_of_period,
        period_frequency,
        reduction_factor,
    }
}

fn market_cap(
    decay: MeteoraDammV2FeeDecay,
    cliff_fee_numerator: u64,
    number_of_period: u16,
    sqrt_price_step_bps: u32,
    scheduler_expiration_duration: u32,
    reduction_factor: u64,
) -> MeteoraDammV2BaseFee {
    MeteoraDammV2BaseFee::MarketCapScheduler {
        decay,
        cliff_fee_numerator,
        number_of_period,
        sqrt_price_step_bps,
        scheduler_expiration_duration,
        reduction_factor,
    }
}

fn fees(
    base_fee: MeteoraDammV2BaseFee,
    compounding_fee_bps: u16,
    dynamic_fee: Option<MeteoraDammV2DynamicFee>,
) -> MeteoraDammV2PoolFees {
    MeteoraDammV2PoolFees {
        base_fee,
        compounding_fee_bps,
        dynamic_fee,
    }
}

/// Against an offset moved in any of the three layouts: a real `pool_fees_raw` per form seen.
#[test]
fn every_creation_layout_seen_decodes_to_its_values() {
    use MeteoraDammV2FeeDecay::{Exponential, Linear};
    let cases = [
        // 13dZFAF2…: mode 0, 50 % over 20 periods of 9.
        (
            "0065cd1d000000001400090000000000000000366e01000000000000000000",
            fees(time(Linear, 500_000_000, 20, 9, 24_000_000), 0, None),
        ),
        // 11L5o1za…: mode 0 at 0 periods, a constant 0.1 %.
        (
            "40420f00000000000000000000000000000000000000000000000000000000",
            fees(time(Linear, 1_000_000, 0, 0, 0), 0, None),
        ),
        // 16LqvgUQ…: mode 0 with a dynamic fee.
        (
            "0065cd1d00000000900032000000000000008db734000000000000000000010100cb10c7bab88d060000000000000000000a007800881360a4dc00ef000000",
            fees(
                time(Linear, 500_000_000, 144, 50, 3_454_861),
                0,
                Some(dynamic_fee(239)),
            ),
        ),
        // 123EHsjJ…: constant 70 % with a dynamic fee.
        (
            "0027b9290000000000000000000000000000000000000000000000000000010100cb10c7bab88d060000000000000000000a007800881360a4dc0000000000",
            fees(time(Linear, 700_000_000, 0, 0, 0), 0, Some(dynamic_fee(0))),
        ),
        // 135x54u5…: mode 1, 50 % over 120 periods of 60.
        (
            "0065cd1d0000000078003c0000000000000009010000000000000100000000",
            fees(time(Exponential, 500_000_000, 120, 60, 265), 0, None),
        ),
        // 19vHgDDD…: mode 1 with a dynamic fee.
        (
            "8033023b00000000b4000100000000000000fc0000000000000001000000010100cb10c7bab88d060000000000000000000a007800881360a4dc00bc030000",
            fees(
                time(Exponential, 990_000_000, 180, 1, 252),
                0,
                Some(dynamic_fee(956)),
            ),
        ),
        // 1wAVvJm5…: mode 3, expiring after 180 days, 80 % compounding.
        (
            "005a620200000000b400a5060000004eed00f32203000000000003401f0000",
            fees(
                market_cap(Linear, 40_000_000, 180, 1701, 15_552_000, 205_555),
                8000,
                None,
            ),
        ),
        // 21BApQbB…: mode 3 with a dynamic fee, expiring after 30 days.
        (
            "0027b929000000001b0083020000008d2700d50964010000000003000000010100cb10c7bab88d060000000000000000000a007800881360a4dc008c050100",
            fees(
                market_cap(Linear, 700_000_000, 27, 643, 2_592_000, 23_333_333),
                0,
                Some(dynamic_fee(66_956)),
            ),
        ),
        // 29gvnHuu…: mode 4.
        (
            "0087930300000000b400a5060000004eed00a5000000000000000470170000",
            fees(
                market_cap(Exponential, 60_000_000, 180, 1701, 15_552_000, 165),
                6000,
                None,
            ),
        ),
        // 267X5pmn…: mode 4 with a dynamic fee.
        (
            "0087930300000000b400a5060000004eed00a50000000000000004000000010100cb10c7bab88d060000000000000000000a007800881360a4dc006b160000",
            fees(
                market_cap(Exponential, 60_000_000, 180, 1701, 15_552_000, 165),
                0,
                Some(dynamic_fee(5739)),
            ),
        ),
        // FPpQsmsc…: 88 % falling to 3 % over 180 one-second periods, 20 % compounding.
        (
            "00bc733400000000b40001000000000000002e0e48000000000000d0070000",
            fees(time(Linear, 880_000_000, 180, 1, 4_722_222), 2000, None),
        ),
    ];
    for (raw, expected) in cases {
        assert_eq!(
            decode_meteora_damm_v2_pool_fees(&bytes(raw)),
            Ok(expected),
            "{raw}"
        );
    }
}

/// Mode 2 has no pool in the snapshot: this blob is built from cp-amm's `BorshFeeRateLimiter`,
/// each field a distinct value so that two swapped offsets cannot agree.
#[test]
fn a_rate_limiter_built_from_the_source_layout_decodes() {
    let mut raw = Vec::new();
    raw.extend(10_000_000u64.to_le_bytes());
    raw.extend(10u16.to_le_bytes());
    raw.extend(3_600u32.to_le_bytes());
    raw.extend(5_000u32.to_le_bytes());
    raw.extend(1_000_000_000u64.to_le_bytes());
    raw.push(2);
    raw.extend([0, 0, 0, 0]);
    assert_eq!(
        decode_meteora_damm_v2_pool_fees(&raw),
        Ok(fees(
            MeteoraDammV2BaseFee::RateLimiter {
                cliff_fee_numerator: 10_000_000,
                fee_increment_bps: 10,
                max_limiter_duration: 3_600,
                max_fee_bps: 5_000,
                reference_amount: 1_000_000_000,
            },
            0,
            None,
        ))
    );
}

/// Against the cliff misread: real `params_raw`, each setting the base fee alone.
#[test]
fn the_fee_updates_of_the_snapshot_decode() {
    let expected = [15_000_000, 859_999_960, 10_000_000, 10_000_000];
    for (raw, cliff) in FEE_UPDATE_OPERATOR_SET.iter().zip(expected) {
        assert_eq!(
            decode_meteora_damm_v2_pool_fees_update(&bytes(raw)),
            Ok(MeteoraDammV2PoolFeesUpdate {
                cliff_fee_numerator: Some(cliff),
                dynamic_fee: MeteoraDammV2DynamicFeeUpdate::Unchanged,
                compounding_fee_bps: None,
            }),
            "{raw}"
        );
    }
}

/// Against the two-field layout refused. The `params_raw` of the fixture
/// `tests/fixtures/damm_v2/update_pool_fees.json`, captured before the compounding fee existed.
#[test]
fn an_update_of_the_earlier_two_field_layout_decodes() {
    let raw = bytes(
        "010050c30000000000010100cb10c7bab88d060000000000000000000a007800881360a4dc00c8040000",
    );
    assert_eq!(
        decode_meteora_damm_v2_pool_fees_update(&raw),
        Ok(MeteoraDammV2PoolFeesUpdate {
            cliff_fee_numerator: Some(12_800_000),
            dynamic_fee: MeteoraDammV2DynamicFeeUpdate::Set(dynamic_fee(1224)),
            compounding_fee_bps: None,
        })
    );
}

/// Against `Disabled` never returned: parameters at their default, all zero, turn it off.
#[test]
fn an_update_with_a_zero_dynamic_fee_disables_it() {
    let mut raw = vec![0, 1];
    raw.extend([0; DYNAMIC_FEE_LEN]);
    raw.extend([1, 0xd0, 0x07]);
    assert_eq!(
        decode_meteora_damm_v2_pool_fees_update(&raw),
        Ok(MeteoraDammV2PoolFeesUpdate {
            cliff_fee_numerator: None,
            dynamic_fee: MeteoraDammV2DynamicFeeUpdate::Disabled,
            compounding_fee_bps: Some(2000),
        })
    );
}

/// One case per refusal, its reason asserted whole.
#[test]
fn a_blob_that_breaks_its_layout_is_refused_with_its_reason() {
    let constant = "40420f00000000000000000000000000000000000000000000000000000000";
    let unknown_mode = format!("{}05{}", &constant[..52], &constant[54..]);
    let bad_dynamic_tag = format!("{}02", &constant[..60]);
    let cases: [(&str, Vec<u8>, Rejection); 5] = [
        (
            "creation, padding 1",
            bytes(&format!("{}01{}", &constant[..58], &constant[60..])),
            Rejection::NonZeroPadding { value: 1 },
        ),
        (
            "creation, mode 5",
            bytes(&unknown_mode),
            Rejection::UnknownBaseFeeMode { mode: 5 },
        ),
        (
            "creation, dynamic tag 2",
            bytes(&bad_dynamic_tag),
            Rejection::InvalidOptionTag {
                field: "dynamic_fee",
                tag: 2,
            },
        ),
        (
            "creation, no dynamic tag",
            bytes(&constant[..60]),
            Rejection::Truncated {
                field: "dynamic_fee",
                len: 0,
                need: 1,
            },
        ),
        (
            "creation, a byte too many",
            bytes(&format!("{constant}00")),
            Rejection::TrailingBytes { len: 1 },
        ),
    ];
    for (case, raw, reason) in cases {
        assert_eq!(
            decode_meteora_damm_v2_pool_fees(&raw),
            Err(reason),
            "{case}"
        );
    }

    let cases: [(&str, Vec<u8>, Rejection); 4] = [
        (
            "update, empty",
            vec![],
            Rejection::Truncated {
                field: "cliff_fee_numerator",
                len: 0,
                need: 1,
            },
        ),
        (
            "update, cliff tag 2",
            bytes("02c0e1e400000000000000"),
            Rejection::InvalidOptionTag {
                field: "cliff_fee_numerator",
                tag: 2,
            },
        ),
        (
            "update, cliff cut short",
            bytes("01c0e1e400"),
            Rejection::Truncated {
                field: "cliff_fee_numerator",
                len: 4,
                need: 8,
            },
        ),
        (
            "update, a byte too many",
            bytes("01c0e1e40000000000000000"),
            Rejection::TrailingBytes { len: 1 },
        ),
    ];
    for (case, raw, reason) in cases {
        assert_eq!(
            decode_meteora_damm_v2_pool_fees_update(&raw),
            Err(reason),
            "{case}"
        );
    }
}
