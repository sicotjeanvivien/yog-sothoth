//! Live integration tests for the DAMM v2 wire event extractor.
//!
//! Each test loads a real Solana transaction (saved as JSON in
//! `tests/fixtures/`) and asserts that the extractor produces the
//! expected wire events.
//!
//! Fixtures are dumped via `solana confirm -v <signature> --output json`
//! against mainnet — they capture the exact shape the RPC returns, so
//! these tests double as regression guards if the JSON schema ever drifts.

use solana_pubkey::{Pubkey, pubkey};
use solana_transaction_status_client_types::EncodedConfirmedTransactionWithStatusMeta;
use std::path::PathBuf;

use yog_core::{
    application::extraction::{
        EventExtractor, MeteoraDammV2,
        meteora::damm_v2::{
            events::DammV2WireEvent,
            extractor::{ExtractFailure, extract_wire_events},
        },
    },
    domain::{DomainEvent, MeteoraDammV2Event, MeteoraDammV2LiquidityEventKind},
};

const CP_AMM_PROGRAM_ID: &str = "cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG";

/// Load and parse a fixture file by name. Panics on any error — fixtures
/// are part of the test contract, missing or malformed ones should fail
/// the test loudly rather than producing confusing assertion errors later.
fn load_fixture(name: &str) -> EncodedConfirmedTransactionWithStatusMeta {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push("fixtures");
    path.push(name);

    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()));

    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("failed to parse fixture {}: {e}", path.display()))
}

/// The reference transaction `2qJrr...` contains two `swap` (legacy)
/// instructions invoking cp-amm on the same pool, in opposite directions.
/// The extractor must surface both as `DammV2WireEvent::Swap2`.
#[test]
fn extracts_both_swaps_from_double_swap_tx() {
    let tx = load_fixture("damm_v2_swap_double.json");

    let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);
    // Sanity: no failure path triggered.
    assert!(
        extracted.failures.is_empty(),
        "unexpected extraction failures: {:?}",
        extracted.failures
    );
    assert!(
        extracted.unknown.is_empty(),
        "unexpected unknown discriminators: {} entries",
        extracted.unknown.len()
    );

    // Two swaps in this transaction — both should round-trip as Swap2.
    assert_eq!(
        extracted.events.len(),
        2,
        "expected 2 events, got {}",
        extracted.events.len()
    );

    for (i, event) in extracted.events.iter().enumerate() {
        assert!(
            matches!(event, DammV2WireEvent::Swap2(_)),
            "event {i} is not a Swap2: {event:?}"
        );
    }
}

/// Decoded values must form a coherent AMM trajectory:
/// the two swaps mutate the pool's reserves in opposite directions, and
/// the transfer amounts match what the user sent / received on-chain.
///
/// Note: `reserve_a_amount` / `reserve_b_amount` in the event reflect the
/// pool's *accounting* reserves (`pool.token_a_amount` / `token_b_amount`),
/// **not** the raw vault balances. The vault balance also includes accrued
/// protocol fees and other components that are tracked separately in the
/// pool state. So we don't compare event reserves to `post_token_balances`.
#[test]
fn decoded_swap_values_match_onchain_reality() {
    let tx = load_fixture("damm_v2_swap_double.json");

    let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);
    assert_eq!(extracted.events.len(), 2, "expected 2 events");
    assert!(extracted.failures.is_empty());
    assert!(extracted.unknown.is_empty());

    let pool_expected = "EgSJAzgCd8oYjMFGqoqtpYFkN3LsBTrbZ5AhACLiFz8G";

    let DammV2WireEvent::Swap2(first) = &extracted.events[0] else {
        panic!("first event is not Swap2");
    };
    let DammV2WireEvent::Swap2(second) = &extracted.events[1] else {
        panic!("second event is not Swap2");
    };

    // Both events refer to the same pool.
    assert_eq!(first.pool.to_string(), pool_expected);
    assert_eq!(second.pool.to_string(), pool_expected);

    // Trade directions:
    // - First swap: SOL in (token_a) → AtoB (0)
    // - Second swap: token in (token_b) → BtoA (1)
    assert_eq!(first.trade_direction, 0, "first swap: expected AtoB (0)");
    assert_eq!(second.trade_direction, 1, "second swap: expected BtoA (1)");

    // Transfer amounts must match the on-chain transferChecked CPIs:
    // - First swap: user sends 9.4 SOL.
    assert_eq!(
        first.included_transfer_fee_amount_in, 9_397_799_749,
        "first swap input amount mismatch"
    );
    // - Second swap: user receives 9.987 SOL.
    assert_eq!(
        second.included_transfer_fee_amount_out, 9_987_369_659,
        "second swap output amount mismatch"
    );

    // Sanity on event reserves: non-zero on both sides for both events.
    assert!(first.reserve_a_amount > 0);
    assert!(first.reserve_b_amount > 0);
    assert!(second.reserve_a_amount > 0);
    assert!(second.reserve_b_amount > 0);

    // AMM invariant: the second swap is BtoA (token in, SOL out), so it
    // must drain reserve_a (SOL) and grow reserve_b (token) compared to
    // the state after the first swap.
    assert!(
        second.reserve_a_amount < first.reserve_a_amount,
        "after BtoA swap, reserve_a should decrease (was {}, now {})",
        first.reserve_a_amount,
        second.reserve_a_amount
    );
    assert!(
        second.reserve_b_amount > first.reserve_b_amount,
        "after BtoA swap, reserve_b should increase (was {}, now {})",
        first.reserve_b_amount,
        second.reserve_b_amount
    );
}

/// `EvtLiquidityChange` — fixtures existed but had no decode test. Validate an
/// add-liquidity tx end-to-end: clean decode, `Add` kind, canonical sorted
/// mints, non-zero amounts/reserves/liquidity_delta, translation preserved.
#[test]
fn decodes_liquidity_add_fixtures() {
    for fixture in ["damm_v2_liquidity_add.json", "damm_v2_liquidity_add_2.json"] {
        let tx = load_fixture(fixture);
        let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);
        assert!(
            extracted.failures.is_empty(),
            "{fixture}: {:?}",
            extracted.failures
        );

        let outcome = MeteoraDammV2::new().extract_events(&tx).expect("extract");
        let liq = outcome
            .events
            .iter()
            .find_map(|e| match e {
                DomainEvent::MeteoraDammV2(MeteoraDammV2Event::Liquidity(e)) => Some(e),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{fixture}: no Liquidity domain event"));

        assert_eq!(
            liq.liquidity_event_kind,
            MeteoraDammV2LiquidityEventKind::Add,
            "{fixture}: expected an Add"
        );
        assert_ne!(
            liq.pool_address,
            Pubkey::default(),
            "{fixture}: pool all-zero"
        );
        assert!(
            liq.reserve_a_after > 0 && liq.reserve_b_after > 0,
            "{fixture}: zero reserves"
        );
        // An add moves at least one side and changes liquidity.
        assert!(
            liq.liquidity_delta > 0,
            "{fixture}: zero liquidity_delta on an add"
        );
        assert!(
            liq.amount_a > 0 || liq.amount_b > 0,
            "{fixture}: no tokens added"
        );
    }
}

#[test]
fn extracts_swap_via_router_correctly() {
    // Fixture: real mainnet transaction where cp-amm is invoked via a router
    // (joeHSutRWndCtp1EPx5tz5zHyaPBZUZ5JsxDEVB1RPZ — Photon-style aggregator).
    //
    // Structure highlight:
    //   - The cp-amm Swap2 outer instruction is itself an inner instruction
    //     of the router (stackHeight 2).
    //   - Both the outer Swap2 and the Anchor event_cpi self-CPI share the
    //     same programId (cp-amm), so distinguishing them by programId alone
    //     is not sufficient — the EVENT_IX_TAG prefix on the self-CPI's data
    //     is what disambiguates.
    //
    // Expected: exactly one EvtSwap2 extracted and successfully translated
    // into a SwapEvent with correct mints (SOL, USDC sorted by raw bytes).

    let json = include_str!("fixtures/damm_v2_swap_via_router.json");
    let tx: EncodedConfirmedTransactionWithStatusMeta =
        serde_json::from_str(json).expect("failed to deserialize transaction");

    let pool = MeteoraDammV2::new();
    let outcome = pool
        .extract_events(&tx)
        .expect("extract_events should succeed at the transaction level");

    // No anchor decode / borsh / translation failures expected.
    assert!(
        outcome.failures.is_empty(),
        "unexpected failures: {:?}",
        outcome.failures
    );

    // Exactly one EvtSwap2 → one DomainEvent::Swap.
    assert_eq!(
        outcome.events.len(),
        1,
        "expected exactly 1 swap event, got {} (events: {:?})",
        outcome.events.len(),
        outcome.events.iter().map(|e| e.kind()).collect::<Vec<_>>()
    );

    let DomainEvent::MeteoraDammV2(MeteoraDammV2Event::Swap(swap)) = &outcome.events[0] else {
        panic!("expected DomainEvent::Swap, got {:?}", outcome.events[0]);
    };

    // Pool address from the EvtSwap2 payload — this is 8Pm2kZ... in the fixture.
    assert_eq!(
        swap.pool_address,
        pubkey!("8Pm2kZpnxD3hoMmt4bjStX2Pw2Z9abpbHzZxMPqxPmie"),
    );

    // Mints are no longer carried on the event (they're a pool property
    // resolved by yog-context), so there's nothing mint-related to assert here.

    // The amounts come from the EvtSwap2 wire fields, mapped via trade_direction.
    // We don't hard-code them here — the EvtSwap2 borsh payload is what drives
    // the values. We just sanity-check they're nonzero.
    assert!(swap.amount_a > 0, "amount_a should be nonzero");
    assert!(swap.amount_b > 0, "amount_b should be nonzero");
    assert!(swap.reserve_a_after > 0);
    assert!(swap.reserve_b_after > 0);
}

/// Both `initialize_pool` fixtures must decode cleanly. This is the guard for
/// the `EvtInitializePool` borsh layout — in particular the nested
/// `PoolFeeParameters` (opaque 27-byte base fee + `Option<DynamicFeeParameters>`):
/// if its layout were wrong, every field after it would be garbage and the
/// borsh deserialize would land in `failures`.
#[test]
fn decodes_initialize_pool_fixtures() {
    for fixture in [
        "damm_v2_initialize_pool.json",
        "damm_v2_initialize_pool_2.json",
        "damm_v2_initialize_pool_3.json",
        "damm_v2_initialize_pool_4.json",
        "damm_v2_initialize_pool_5.json",
        "damm_v2_initialize_pool_6.json",
    ] {
        let tx = load_fixture(fixture);
        let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);

        assert!(
            extracted.failures.is_empty(),
            "{fixture}: unexpected extraction failures (likely a wire-layout mismatch): {:?}",
            extracted.failures
        );

        let init = extracted
            .events
            .iter()
            .find_map(|e| match e {
                DammV2WireEvent::InitializePool(e) => Some(e),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{fixture}: no InitializePool event extracted"));

        // Coherence checks — these only hold if the layout decoded correctly.
        assert_ne!(
            init.token_a_mint, init.token_b_mint,
            "{fixture}: the two mints must differ"
        );
        assert_ne!(
            init.token_a_mint,
            Pubkey::default(),
            "{fixture}: token_a_mint is all-zero — layout drift"
        );
        assert!(
            init.sqrt_price > 0,
            "{fixture}: sqrt_price should be non-zero"
        );
        assert!(
            init.sqrt_min_price < init.sqrt_max_price,
            "{fixture}: sqrt_min_price ({}) must be < sqrt_max_price ({})",
            init.sqrt_min_price,
            init.sqrt_max_price
        );
        // Small enums: garbage bytes from a layout drift would blow these up.
        assert!(
            init.collect_fee_mode <= 2,
            "{fixture}: collect_fee_mode out of range: {}",
            init.collect_fee_mode
        );
        assert!(
            init.pool_type <= 4,
            "{fixture}: pool_type out of range: {}",
            init.pool_type
        );
        assert!(
            init.activation_type <= 2,
            "{fixture}: activation_type out of range: {}",
            init.activation_type
        );
        // NOTE: cp-amm does NOT emit mints sorted by raw bytes — the event
        // preserves the program's native token_a/token_b designation. The
        // `initialize_pool_events` table stores this native order as-is; the
        // pool *registry* upsert re-sorts to the canonical convention shared
        // with the swap/liquidity tables (see persist_initialize_pool).
    }
}

/// `decode_fee_config` must run cleanly on the **real** genesis blobs (not
/// hand-built bytes), and classify each fixture's fee shape correctly. This
/// ties the decoder to live data: a `PoolFeeParameters` layout drift would
/// break the classification here even if the standalone unit tests (which use
/// captured byte arrays) stayed green.
#[test]
fn decode_fee_config_matches_real_genesis_fixtures() {
    use yog_core::amm::damm_v2::{BaseFeeKind, FeeConfig, decode_fee_config};

    // (fixture, expected) — real-data coverage of ALL FOUR base_fee_kind
    // variants: Constant (2/3/4), SchedulerLinear (1), SchedulerExponential
    // (5), RateLimiter (6); and both has_dynamic_fee values (false only on 5).
    let cases = [
        (
            "damm_v2_initialize_pool.json",
            FeeConfig {
                base_kind: BaseFeeKind::SchedulerLinear,
                has_dynamic_fee: true,
            },
        ),
        (
            "damm_v2_initialize_pool_2.json",
            FeeConfig {
                base_kind: BaseFeeKind::Constant,
                has_dynamic_fee: true,
            },
        ),
        (
            "damm_v2_initialize_pool_3.json",
            FeeConfig {
                base_kind: BaseFeeKind::Constant,
                has_dynamic_fee: true,
            },
        ),
        // fixture_4: distinct real tx, but its fee sub-blob is byte-identical
        // to fixture_3 (constant 100 bps + dynamic) — no new fee-config case,
        // kept as another extraction/layout data point.
        (
            "damm_v2_initialize_pool_4.json",
            FeeConfig {
                base_kind: BaseFeeKind::Constant,
                has_dynamic_fee: true,
            },
        ),
        // fixture_5: the first real-data validation of two branches previously
        // covered only synthetically — the exponential scheduler AND a pool
        // with NO dynamic fee (Option tag 0, so the blob ends at 31 bytes, no
        // trailing DynamicFeeParameters — exercising the length boundary too).
        (
            "damm_v2_initialize_pool_5.json",
            FeeConfig {
                base_kind: BaseFeeKind::SchedulerExponential,
                has_dynamic_fee: false,
            },
        ),
        // fixture_6: the first real-data rate limiter (mode 2, cliff 4%). Its
        // bytes 8..26 are reinterpreted rate-limiter params — decode_fee_config
        // must classify it as RateLimiter WITHOUT reading them as scheduler
        // fields (and decode_base_fee_bps still reads the shared leading u64).
        (
            "damm_v2_initialize_pool_6.json",
            FeeConfig {
                base_kind: BaseFeeKind::RateLimiter,
                has_dynamic_fee: true,
            },
        ),
    ];

    for (fixture, expected) in cases {
        let tx = load_fixture(fixture);
        let init = extract_wire_events(&tx, CP_AMM_PROGRAM_ID)
            .events
            .into_iter()
            .find_map(|e| match e {
                DammV2WireEvent::InitializePool(e) => Some(e),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{fixture}: no InitializePool event"));

        let raw = borsh::to_vec(&init.pool_fees).expect("borsh serialize is infallible");
        assert_eq!(
            decode_fee_config(&raw).unwrap(),
            expected,
            "{fixture}: unexpected fee config"
        );
    }
}

/// `EvtCreatePosition` rides along in the genesis transactions (a pool is
/// created and its first position opened together), so the initialize_pool
/// fixtures double as real-data validation for it: clean decode, sane fields,
/// and a full translation into the domain event.
#[test]
fn decodes_create_position_from_genesis_fixtures() {
    for fixture in [
        "damm_v2_initialize_pool.json",
        "damm_v2_initialize_pool_2.json",
    ] {
        let tx = load_fixture(fixture);
        let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);
        assert!(
            extracted.failures.is_empty(),
            "{fixture}: failures: {:?}",
            extracted.failures
        );

        let create = extracted
            .events
            .iter()
            .find_map(|e| match e {
                DammV2WireEvent::CreatePosition(e) => Some(e),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{fixture}: no CreatePosition event"));

        assert_ne!(create.pool, Pubkey::default(), "{fixture}: pool all-zero");
        assert_ne!(create.owner, Pubkey::default(), "{fixture}: owner all-zero");
        assert_ne!(
            create.position,
            Pubkey::default(),
            "{fixture}: position all-zero"
        );
        assert_ne!(
            create.position_nft_mint,
            Pubkey::default(),
            "{fixture}: position_nft_mint all-zero"
        );
        assert_ne!(
            create.position, create.position_nft_mint,
            "{fixture}: position and its NFT mint must differ"
        );

        // Full pipeline: wire → domain, fields preserved.
        let (pool, owner, position, nft) = (
            create.pool,
            create.owner,
            create.position,
            create.position_nft_mint,
        );
        let outcome = MeteoraDammV2::new().extract_events(&tx).expect("extract");
        let domain = outcome
            .events
            .iter()
            .find_map(|e| match e {
                DomainEvent::MeteoraDammV2(MeteoraDammV2Event::CreatePosition(e)) => Some(e),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{fixture}: no CreatePosition domain event"));
        assert_eq!(domain.pool_address, pool);
        assert_eq!(domain.owner, owner);
        assert_eq!(domain.position, position);
        assert_eq!(domain.position_nft_mint, nft);
    }
}

/// `EvtClosePosition` — same 4-pubkey shape as create. Validate against a real
/// close transaction: clean decode, sane distinct fields, translation preserved.
#[test]
fn decodes_close_position_fixture() {
    let tx = load_fixture("damm_v2_close_position.json");
    let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);
    assert!(extracted.failures.is_empty(), "{:?}", extracted.failures);

    let close = extracted
        .events
        .iter()
        .find_map(|e| match e {
            DammV2WireEvent::ClosePosition(e) => Some(e),
            _ => None,
        })
        .expect("no ClosePosition event");

    assert_ne!(close.pool, Pubkey::default());
    assert_ne!(close.owner, Pubkey::default());
    assert_ne!(close.position, Pubkey::default());
    assert_ne!(close.position_nft_mint, Pubkey::default());
    assert_ne!(close.position, close.position_nft_mint);

    let (pool, owner, position, nft) = (
        close.pool,
        close.owner,
        close.position,
        close.position_nft_mint,
    );
    let outcome = MeteoraDammV2::new().extract_events(&tx).expect("extract");
    let domain = outcome
        .events
        .iter()
        .find_map(|e| match e {
            DomainEvent::MeteoraDammV2(MeteoraDammV2Event::ClosePosition(e)) => Some(e),
            _ => None,
        })
        .expect("no ClosePosition domain event");
    assert_eq!(domain.pool_address, pool);
    assert_eq!(domain.owner, owner);
    assert_eq!(domain.position, position);
    assert_eq!(domain.position_nft_mint, nft);
}

/// `EvtLockPosition` — non-trivial layout (4 pubkeys + u64×2 + u128×2 + u16).
/// A layout drift would scramble the pubkeys or fail borsh; the vesting
/// numerics are checked for coherence and round-tripped through the domain.
#[test]
fn decodes_lock_position_fixture() {
    let tx = load_fixture("damm_v2_lock_position.json");
    let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);
    assert!(extracted.failures.is_empty(), "{:?}", extracted.failures);

    let lock = extracted
        .events
        .iter()
        .find_map(|e| match e {
            DammV2WireEvent::LockPosition(e) => Some(e),
            _ => None,
        })
        .expect("no LockPosition event");

    for (p, name) in [
        (lock.pool, "pool"),
        (lock.position, "position"),
        (lock.owner, "owner"),
        (lock.vesting, "vesting"),
    ] {
        assert_ne!(p, Pubkey::default(), "{name} is all-zero — layout drift");
    }
    // A lock must immobilise some liquidity, either fully at the cliff
    // (number_of_period == 0, a valid cliff-only lock) or spread over periods.
    // Garbage from a misaligned u128 region would not satisfy this.
    assert!(
        lock.cliff_unlock_liquidity > 0 || lock.liquidity_per_period > 0,
        "no liquidity locked across cliff or periods"
    );

    let snapshot = (
        lock.pool,
        lock.position,
        lock.owner,
        lock.vesting,
        lock.cliff_point,
        lock.period_frequency,
        lock.cliff_unlock_liquidity,
        lock.liquidity_per_period,
        lock.number_of_period,
    );
    let outcome = MeteoraDammV2::new().extract_events(&tx).expect("extract");
    let d = outcome
        .events
        .iter()
        .find_map(|e| match e {
            DomainEvent::MeteoraDammV2(MeteoraDammV2Event::LockPosition(e)) => Some(e),
            _ => None,
        })
        .expect("no LockPosition domain event");
    assert_eq!(
        snapshot,
        (
            d.pool_address,
            d.position,
            d.owner,
            d.vesting,
            d.cliff_point,
            d.period_frequency,
            d.cliff_unlock_liquidity,
            d.liquidity_per_period,
            d.number_of_period,
        )
    );
}

/// `EvtPermanentLockPosition` — pubkey×2 + u128×2. The running total must be
/// at least the amount locked by this action: a structural invariant that a
/// scrambled layout would almost certainly violate.
#[test]
fn decodes_permanent_lock_position_fixture() {
    let tx = load_fixture("damm_v2_permanent_lock_position.json");
    let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);
    // This tx also contains an 8-byte tag-only cp-amm self-CPI that trips the
    // anchor decoder (a benign, pre-existing skip-and-log case). Tolerate that
    // AnchorDecode failure, but a Borsh failure would mean a *recognized* event
    // decoded wrong — that must not happen.
    assert!(
        !extracted
            .failures
            .iter()
            .any(|f| matches!(f, ExtractFailure::Borsh { .. })),
        "unexpected Borsh failures: {:?}",
        extracted.failures
    );

    let plock = extracted
        .events
        .iter()
        .find_map(|e| match e {
            DammV2WireEvent::PermanentLockPosition(e) => Some(e),
            _ => None,
        })
        .expect("no PermanentLockPosition event");

    assert_ne!(plock.pool, Pubkey::default());
    assert_ne!(plock.position, Pubkey::default());
    assert!(plock.lock_liquidity_amount > 0, "nothing locked");
    assert!(
        plock.total_permanent_locked_liquidity >= plock.lock_liquidity_amount,
        "running total ({}) < this lock ({}) — layout drift",
        plock.total_permanent_locked_liquidity,
        plock.lock_liquidity_amount
    );

    let snapshot = (
        plock.pool,
        plock.position,
        plock.lock_liquidity_amount,
        plock.total_permanent_locked_liquidity,
    );
    let outcome = MeteoraDammV2::new().extract_events(&tx).expect("extract");
    let d = outcome
        .events
        .iter()
        .find_map(|e| match e {
            DomainEvent::MeteoraDammV2(MeteoraDammV2Event::PermanentLockPosition(e)) => Some(e),
            _ => None,
        })
        .expect("no PermanentLockPosition domain event");
    assert_eq!(
        snapshot,
        (
            d.pool_address,
            d.position,
            d.lock_liquidity_amount,
            d.total_permanent_locked_liquidity,
        )
    );
}

/// `EvtClaimPositionFee` — validate against a real claim: clean decode, sane
/// pubkeys, and a full wire→domain translation preserving every field.
#[test]
fn decodes_claim_position_fee_fixture() {
    let tx = load_fixture("damm_v2_claim_position_fee.json");
    let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);
    assert!(
        !extracted
            .failures
            .iter()
            .any(|f| matches!(f, ExtractFailure::Borsh { .. })),
        "unexpected Borsh failures: {:?}",
        extracted.failures
    );

    let claim = extracted
        .events
        .iter()
        .find_map(|e| match e {
            DammV2WireEvent::ClaimPositionFee(e) => Some(e),
            _ => None,
        })
        .expect("no ClaimPositionFee event");
    assert_ne!(claim.pool, Pubkey::default());
    assert_ne!(claim.position, Pubkey::default());
    assert_ne!(claim.owner, Pubkey::default());

    let outcome = MeteoraDammV2::new().extract_events(&tx).expect("extract");
    let d = outcome
        .events
        .iter()
        .find_map(|e| match e {
            DomainEvent::MeteoraDammV2(MeteoraDammV2Event::ClaimPositionFee(e)) => Some(e),
            _ => None,
        })
        .expect("no ClaimPositionFee domain event");
    assert_eq!(d.pool_address, claim.pool);
    assert_eq!(d.position, claim.position);
    assert_eq!(d.owner, claim.owner);
    assert_eq!(d.fee_a_claimed, claim.fee_a_claimed);
    assert_eq!(d.fee_b_claimed, claim.fee_b_claimed);
}

/// `EvtClaimReward` — same shape of guard: decode + field-preserving translation
/// on a real reward claim. `reward_index` disambiguates the reward stream.
#[test]
fn decodes_claim_reward_fixture() {
    let tx = load_fixture("damm_v2_claim_reward.json");
    let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);
    assert!(
        !extracted
            .failures
            .iter()
            .any(|f| matches!(f, ExtractFailure::Borsh { .. })),
        "unexpected Borsh failures: {:?}",
        extracted.failures
    );

    let claim = extracted
        .events
        .iter()
        .find_map(|e| match e {
            DammV2WireEvent::ClaimReward(e) => Some(e),
            _ => None,
        })
        .expect("no ClaimReward event");
    assert_ne!(claim.pool, Pubkey::default());
    assert_ne!(claim.position, Pubkey::default());
    assert_ne!(claim.owner, Pubkey::default());
    assert_ne!(claim.mint_reward, Pubkey::default());

    let outcome = MeteoraDammV2::new().extract_events(&tx).expect("extract");
    let d = outcome
        .events
        .iter()
        .find_map(|e| match e {
            DomainEvent::MeteoraDammV2(MeteoraDammV2Event::ClaimReward(e)) => Some(e),
            _ => None,
        })
        .expect("no ClaimReward domain event");
    assert_eq!(d.pool_address, claim.pool);
    assert_eq!(d.position, claim.position);
    assert_eq!(d.owner, claim.owner);
    assert_eq!(d.mint_reward, claim.mint_reward);
    assert_eq!(d.reward_index, claim.reward_index);
    assert_eq!(d.total_reward, claim.total_reward);
}

/// Guard for the `EvtUpdatePoolFees` decode. Its `BorshDeserialize` is custom:
/// it reads the two leading pubkeys (pool, operator) and captures the trailing
/// `UpdatePoolFeesParameters` bytes verbatim into `params_raw` ("voie C"). A
/// clean decode here proves the discriminator matches and the prefix layout
/// (pool, operator) is correct on a real on-chain transaction.
#[test]
fn decodes_update_pool_fees_fixture() {
    let tx = load_fixture("damm_v2_update_pool_fees.json");
    let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);

    assert!(
        extracted.failures.is_empty(),
        "unexpected extraction failures: {:?}",
        extracted.failures
    );

    let update = extracted
        .events
        .iter()
        .find_map(|e| match e {
            DammV2WireEvent::UpdatePoolFees(e) => Some(e),
            _ => None,
        })
        .expect("no UpdatePoolFees event extracted");

    assert_ne!(
        update.pool,
        Pubkey::default(),
        "pool is all-zero — discriminator matched but prefix layout is wrong"
    );
    assert_ne!(
        update.operator,
        Pubkey::default(),
        "operator is all-zero — prefix layout drift"
    );
    // The trailing params blob is captured verbatim; a real fee update carries
    // a non-empty UpdatePoolFeesParameters.
    assert!(
        !update.params_raw.is_empty(),
        "params_raw should be non-empty"
    );

    // End-to-end: the full extractor must translate it into the domain event,
    // preserving pool / operator / the raw params blob.
    let (wire_pool, wire_operator, wire_params) =
        (update.pool, update.operator, update.params_raw.clone());

    let outcome = MeteoraDammV2::new()
        .extract_events(&tx)
        .expect("extract_events should succeed at the transaction level");
    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);

    let domain = outcome
        .events
        .iter()
        .find_map(|e| match e {
            DomainEvent::MeteoraDammV2(MeteoraDammV2Event::UpdatePoolFees(e)) => Some(e),
            _ => None,
        })
        .expect("no UpdatePoolFees domain event produced");

    assert_eq!(domain.pool_address, wire_pool);
    assert_eq!(domain.operator, wire_operator);
    assert_eq!(domain.params_raw, wire_params);
}
