//! The DAMM v2 extraction pipeline on real mainnet transactions (`solana confirm -v <sig>
//! --output json`), read by path from `yog-core`'s fixtures: one verbatim copy, no drift.

use solana_pubkey::{Pubkey, pubkey};
use solana_transaction_status_client_types::EncodedConfirmedTransactionWithStatusMeta;
use std::path::PathBuf;

use yog_core::{
    AnchorDecodeError,
    application::extraction::{
        EventExtractor, ExtractionFailure, MeteoraDammV2, OnChainTransaction,
        meteora::damm_v2::{
            events::DammV2WireEvent,
            extractor::{ExtractFailure, extract_wire_events},
        },
    },
    domain::{DomainEvent, MeteoraDammV2Event, MeteoraDammV2LiquidityEventKind},
};

use super::from_rpc;

const CP_AMM_PROGRAM_ID: Pubkey = pubkey!("cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG");

/// Load a fixture and hand it to the pipeline as the JSON-RPC adapter fills it; panics on any
/// error, a fixture being part of the test contract.
fn load_fixture(name: &str) -> OnChainTransaction {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../core/tests/fixtures");
    path.push(name);

    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()));

    let tx: EncodedConfirmedTransactionWithStatusMeta = serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("failed to parse fixture {}: {e}", path.display()));

    from_rpc(&tx).unwrap_or_else(|e| panic!("failed to adapt fixture {}: {e}", path.display()))
}

/// The reference transaction `2qJrr...`: two legacy `swap`s on one pool, opposite directions,
/// both surfaced as `DammV2WireEvent::Swap2`.
#[test]
fn extracts_both_swaps_from_double_swap_tx() {
    let tx = load_fixture("damm_v2/swap_double.json");

    let extracted = extract_wire_events(&tx, &CP_AMM_PROGRAM_ID);
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

    for (i, indexed) in extracted.events.iter().enumerate() {
        assert!(
            matches!(indexed.event, DammV2WireEvent::Swap2(_)),
            "event {i} is not a Swap2: {indexed:?}"
        );
    }
}

/// The two swaps form a coherent trajectory, and the transfer amounts match the chain.
/// ⚠️ Event reserves are the pool's accounting reserves, not vault balances (which hold accrued
/// fees too): never compared to `post_token_balances`.
#[test]
fn decoded_swap_values_match_onchain_reality() {
    let tx = load_fixture("damm_v2/swap_double.json");

    let extracted = extract_wire_events(&tx, &CP_AMM_PROGRAM_ID);
    assert_eq!(extracted.events.len(), 2, "expected 2 events");
    assert!(extracted.failures.is_empty());
    assert!(extracted.unknown.is_empty());

    let pool_expected = "EgSJAzgCd8oYjMFGqoqtpYFkN3LsBTrbZ5AhACLiFz8G";

    let DammV2WireEvent::Swap2(first) = &extracted.events[0].event else {
        panic!("first event is not Swap2");
    };
    let DammV2WireEvent::Swap2(second) = &extracted.events[1].event else {
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

/// An add-liquidity transaction end to end: clean decode, `Add` kind, non-zero amounts, reserves
/// and liquidity delta, translation preserved.
#[test]
fn decodes_liquidity_add_fixtures() {
    for fixture in ["damm_v2/liquidity_add.json", "damm_v2/liquidity_add_2.json"] {
        let tx = load_fixture(fixture);
        let extracted = extract_wire_events(&tx, &CP_AMM_PROGRAM_ID);
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

/// A version 1 transaction (`2yyZaQ3w…`, a Jupiter route) reads like a legacy one: its
/// `transactionConfig` ignored, its `EvtSwap2` extracted, its pool the swap's second account.
#[test]
fn a_v1_transaction_yields_its_swap() {
    let tx = load_fixture("damm_v2/swap_v1.json");

    let outcome = MeteoraDammV2::new()
        .extract_events(&tx)
        .expect("a v1 transaction must extract at the transaction level");

    assert!(
        outcome.failures.is_empty(),
        "unexpected failures: {:?}",
        outcome.failures
    );
    assert_eq!(
        outcome.events.len(),
        1,
        "expected the transaction's one EvtSwap2, got {:?}",
        outcome.events.iter().map(|e| e.kind()).collect::<Vec<_>>()
    );

    let DomainEvent::MeteoraDammV2(MeteoraDammV2Event::Swap(swap)) = &outcome.events[0] else {
        panic!("expected DomainEvent::Swap, got {:?}", outcome.events[0]);
    };
    assert_eq!(
        swap.pool_address,
        pubkey!("8Pm2kZpnxD3hoMmt4bjStX2Pw2Z9abpbHzZxMPqxPmie"),
    );
    assert_eq!(tx.position.slot, 447_166_931);
    assert_eq!(tx.position.transaction_index, Some(544));
}

#[test]
fn extracts_swap_via_router_correctly() {
    // A router (`joeHSut…`) invokes cp-amm, so the outer Swap2 is itself inner (stackHeight 2) and
    // shares the self-CPI's programId: only the `EVENT_IX_TAG` prefix tells them apart.

    let tx = load_fixture("damm_v2/swap_via_router.json");

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

    // The amounts come from the EvtSwap2 payload; only checked non-zero.
    assert!(swap.amount_a > 0, "amount_a should be nonzero");
    assert!(swap.amount_b > 0, "amount_b should be nonzero");
    assert!(swap.reserve_a_after > 0);
    assert!(swap.reserve_b_after > 0);
}

/// Both `initialize_pool` fixtures decode cleanly: the guard for `EvtInitializePool`'s layout,
/// whose nested `PoolFeeParameters` would leave every later field garbage if wrong.
#[test]
fn decodes_initialize_pool_fixtures() {
    for fixture in [
        "damm_v2/initialize_pool.json",
        "damm_v2/initialize_pool_2.json",
        "damm_v2/initialize_pool_3.json",
        "damm_v2/initialize_pool_4.json",
        "damm_v2/initialize_pool_5.json",
        "damm_v2/initialize_pool_6.json",
    ] {
        let tx = load_fixture(fixture);
        let extracted = extract_wire_events(&tx, &CP_AMM_PROGRAM_ID);

        assert!(
            extracted.failures.is_empty(),
            "{fixture}: unexpected extraction failures (likely a wire-layout mismatch): {:?}",
            extracted.failures
        );

        let init = extracted
            .events
            .iter()
            .find_map(|e| match &e.event {
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
        // ⚠️ cp-amm keeps its own token_a/token_b order, unsorted, and nothing downstream sorts
        // it: yog-context fills `pools`' mints from the account in that same order.
    }
}

/// The claim-protocol-fee fixture decodes to an `EvtClaimProtocolFee`, the `emit_cpi!` variant.
#[test]
fn decodes_claim_protocol_fee_fixture() {
    let tx = load_fixture("damm_v2/claim_protocol_fee.json");
    let extracted = extract_wire_events(&tx, &CP_AMM_PROGRAM_ID);

    assert!(
        extracted.failures.is_empty(),
        "unexpected extraction failures: {:?}",
        extracted.failures
    );

    let cpf = extracted
        .events
        .iter()
        .find_map(|e| match &e.event {
            DammV2WireEvent::ClaimProtocolFee(e) => Some(e),
            _ => None,
        })
        .expect("no ClaimProtocolFee event extracted");

    assert_ne!(
        cpf.pool,
        Pubkey::default(),
        "pool all-zero — wire layout drift"
    );
    // A real withdrawal transfers at least one side (this fixture: only B).
    assert!(
        cpf.token_a_amount > 0 || cpf.token_b_amount > 0,
        "both claimed amounts zero — layout drift"
    );
}

/// The initialize-reward fixture decodes to an `EvtInitializeReward`, every field pinned; it also
/// funds the slot. ⚠️ `funder` and `creator` are one wallet here: their order comes from cp-amm.
#[test]
fn decodes_initialize_reward_fixture() {
    let tx = load_fixture("damm_v2/initialize_reward.json");
    let extracted = extract_wire_events(&tx, &CP_AMM_PROGRAM_ID);

    assert!(
        extracted.failures.is_empty(),
        "unexpected extraction failures: {:?}",
        extracted.failures
    );

    let ir = extracted
        .events
        .iter()
        .find_map(|e| match &e.event {
            DammV2WireEvent::InitializeReward(e) => Some(e),
            _ => None,
        })
        .expect("no InitializeReward event extracted");

    assert_eq!(
        ir.pool,
        pubkey!("8UBoT36exEV5g7GyzfrR6YMmKL2SMhdFNTqjTbP9vJZt")
    );
    assert_eq!(
        ir.reward_mint,
        pubkey!("KMtG8obzMXtJkL6EKwd6faxvgyRWxBHwd68SQv9PLEX")
    );
    assert_eq!(
        ir.funder,
        pubkey!("HkcPD14egD2cR4Rd189zqbxYGmyanbPGUFLVH4zFcYXe")
    );
    assert_eq!(
        ir.creator,
        pubkey!("HkcPD14egD2cR4Rd189zqbxYGmyanbPGUFLVH4zFcYXe")
    );
    assert_eq!(ir.reward_index, 0);
    // 604800 s = 7 days. A one-byte misalignment upstream would wreck this.
    assert_eq!(ir.reward_duration, 604_800);
}

/// The fund-reward fixtures decode to an `EvtFundReward`: a first funding (`initialize_reward.json`)
/// and a re-fund of a live slot that carries the remainder forward (`fund_reward.json`).
#[test]
fn decodes_fund_reward_fixtures() {
    // Fresh slot: opened and funded in one transaction.
    let tx = load_fixture("damm_v2/initialize_reward.json");
    let extracted = extract_wire_events(&tx, &CP_AMM_PROGRAM_ID);
    assert!(
        extracted.failures.is_empty(),
        "unexpected extraction failures: {:?}",
        extracted.failures
    );

    let fr = extracted
        .events
        .iter()
        .find_map(|e| match &e.event {
            DammV2WireEvent::FundReward(e) => Some(e),
            _ => None,
        })
        .expect("no FundReward event extracted from the initialize fixture");

    assert_eq!(
        fr.pool,
        pubkey!("8UBoT36exEV5g7GyzfrR6YMmKL2SMhdFNTqjTbP9vJZt")
    );
    assert_eq!(
        fr.funder,
        pubkey!("HkcPD14egD2cR4Rd189zqbxYGmyanbPGUFLVH4zFcYXe")
    );
    assert_eq!(
        fr.mint_reward,
        pubkey!("KMtG8obzMXtJkL6EKwd6faxvgyRWxBHwd68SQv9PLEX")
    );
    assert_eq!(fr.reward_index, 0);
    assert_eq!(fr.amount, 100_000_000_000);
    // No Token-2022 transfer fee on this mint: everything sent landed.
    assert_eq!(fr.transfer_fee_excluded_amount_in, fr.amount);
    // Slot opened at blockTime 1785122388 with a 604800 s window.
    assert_eq!(fr.reward_duration_end, 1_785_727_188);
    // First funding of the slot — nothing was emitting before.
    assert_eq!(fr.pre_reward_rate, 0);

    // The rate is Q64.64: on a fresh slot it must equal (amount << 64) / duration exactly, which
    // a wrong scale or a misread u128 breaks and a non-zero check would not.
    let duration: u128 = 604_800;
    assert_eq!(
        fr.post_reward_rate,
        (u128::from(fr.amount) << 64) / duration,
        "post_reward_rate is not (amount << 64) / duration — Q64.64 scale drift"
    );

    // Re-fund of a live slot: carry-forward regime.
    let tx = load_fixture("damm_v2/fund_reward.json");
    let extracted = extract_wire_events(&tx, &CP_AMM_PROGRAM_ID);
    assert!(
        extracted.failures.is_empty(),
        "unexpected extraction failures: {:?}",
        extracted.failures
    );

    let fr = extracted
        .events
        .iter()
        .find_map(|e| match &e.event {
            DammV2WireEvent::FundReward(e) => Some(e),
            _ => None,
        })
        .expect("no FundReward event extracted");

    assert_eq!(
        fr.pool,
        pubkey!("C21PsA1opG7wogPyk8vYjKw7K1J8hooDrK3rHg6LTThV")
    );
    assert_eq!(
        fr.funder,
        pubkey!("HkRUrQh9H6s129v1DP5xFwL5uNsmfS3STTbGEKwehcxt")
    );
    assert_eq!(
        fr.mint_reward,
        pubkey!("DwAgR3U6tDoQMiBtJq3YVjoeK6hapYvxc8ZHeAQgjups")
    );
    assert_eq!(fr.reward_index, 0);
    assert_eq!(fr.amount, 29_529_966_570);
    assert_eq!(fr.transfer_fee_excluded_amount_in, fr.amount);
    assert_eq!(fr.reward_duration_end, 1_785_711_398);
    // A live slot was already emitting, and topping it up raised the rate.
    assert!(fr.pre_reward_rate > 0, "expected a live slot");
    assert!(
        fr.post_reward_rate > fr.pre_reward_rate,
        "funding a slot must not lower its emission rate"
    );
    // Carry-forward: the new window distributes more than this deposit; total = rate * duration >> 64.
    let total = (fr.post_reward_rate * duration) >> 64;
    assert!(
        total > u128::from(fr.amount),
        "expected carry-forward: total ({total}) should exceed the deposit ({})",
        fr.amount
    );
}

/// The withdraw-ineligible-reward fixture decodes to an `EvtWithdrawIneligibleReward`.
/// ⚠️ Its `amount` is legitimately zero: pinned but not discriminating; the two pubkeys are.
#[test]
fn decodes_withdraw_ineligible_reward_fixture() {
    let tx = load_fixture("damm_v2/withdraw_ineligible_reward.json");
    let extracted = extract_wire_events(&tx, &CP_AMM_PROGRAM_ID);

    assert!(
        extracted.failures.is_empty(),
        "unexpected extraction failures: {:?}",
        extracted.failures
    );

    let wir = extracted
        .events
        .iter()
        .find_map(|e| match &e.event {
            DammV2WireEvent::WithdrawIneligibleReward(e) => Some(e),
            _ => None,
        })
        .expect("no WithdrawIneligibleReward event extracted");

    assert_eq!(
        wir.pool,
        pubkey!("2jmzTgpVpPdDbHWJBeJHYSbY4YFiycdbXuovAGBrGMVp")
    );
    assert_eq!(
        wir.reward_mint,
        pubkey!("venum8bXEakzg6eSQWikT51EoeuAsR51Wxn7JfQc4QX")
    );
    // Pinned, not asserted non-zero: nothing was reclaimable.
    assert_eq!(wir.amount, 0);
}

/// A `zap_protocol_fee` transaction yields exactly one event, and not the harvest: that emits
/// none. Its `EvtSwap2` is a Jupiter route through an unrelated cp-amm pool, asserted different.
/// ⚠️ The two non-`event_cpi` cp-amm calls must be skipped, neither unknown nor failures.
#[test]
fn zap_protocol_fee_emits_no_event_of_its_own() {
    let tx = load_fixture("damm_v2/zap_protocol_fee.json");
    let extracted = extract_wire_events(&tx, &CP_AMM_PROGRAM_ID);

    assert!(
        extracted.failures.is_empty(),
        "non-event cp-amm instructions must be skipped, not failed: {:?}",
        extracted.failures
    );
    assert!(
        extracted.unknown.is_empty(),
        "non-event cp-amm instructions must not surface as unknown discriminators: {} entries",
        extracted.unknown.len()
    );
    assert_eq!(
        extracted.events.len(),
        1,
        "expected exactly one event (the Jupiter-routed swap), got {}",
        extracted.events.len()
    );

    let DammV2WireEvent::Swap2(swap) = &extracted.events[0].event else {
        panic!("the only event must be a Swap2: {:?}", extracted.events[0]);
    };
    // The pool Jupiter routed through, not one `zap_protocol_fee` harvested from.
    assert_eq!(
        swap.pool,
        pubkey!("4C47JJgDHztupNHeU9da7MroH676eXbPoUhEvx3FpfR3")
    );
    // An ordinary swap in every respect: real amounts, real post-swap reserves.
    assert_eq!(swap.included_transfer_fee_amount_in, 1_228_454_277_679);
    assert_eq!(swap.included_transfer_fee_amount_out, 1_615_941_369);
    assert!(swap.reserve_a_amount > 0 && swap.reserve_b_amount > 0);
}

/// The split fixtures decode to `EvtSplitPosition3`, the deprecated `EvtSplitPosition2` emitted
/// beside it dropped silently: the double-counting guard. `split_position2.json` also creates a position.
#[test]
fn decodes_split_position_fixtures_and_drops_the_deprecated_v2() {
    for fixture in [
        "damm_v2/split_position.json",
        "damm_v2/split_position2.json",
    ] {
        let tx = load_fixture(fixture);
        let extracted = extract_wire_events(&tx, &CP_AMM_PROGRAM_ID);

        assert!(
            extracted.failures.is_empty(),
            "{fixture}: {:?}",
            extracted.failures
        );
        // `EvtSplitPosition2` comes with every split: here, the drop stopped working.
        assert!(
            extracted.unknown.is_empty(),
            "{fixture}: the deprecated EvtSplitPosition2 must be dropped, not \
             reported as unknown ({} entries)",
            extracted.unknown.len()
        );

        let splits: Vec<_> = extracted
            .events
            .iter()
            .filter_map(|e| match &e.event {
                DammV2WireEvent::SplitPosition3(e) => Some(e),
                _ => None,
            })
            .collect();
        assert_eq!(
            splits.len(),
            1,
            "{fixture}: expected exactly one split event, got {}",
            splits.len()
        );
        let sp = splits[0];

        assert_ne!(sp.pool, Pubkey::default(), "{fixture}: pool all-zero");
        assert_ne!(
            sp.first_position, sp.second_position,
            "{fixture}: a split must target two distinct positions"
        );
        assert!(
            sp.current_sqrt_price > 0,
            "{fixture}: sqrt_price should be non-zero"
        );
        // Numerators are fractions over SPLIT_POSITION_DENOMINATOR = 1e9, validated on-chain.
        const DENOMINATOR: u32 = 1_000_000_000;
        let n = &sp.split_position_parameters;
        for (label, value) in [
            ("unlocked_liquidity", n.unlocked_liquidity_numerator),
            (
                "permanent_locked_liquidity",
                n.permanent_locked_liquidity_numerator,
            ),
            ("fee_a", n.fee_a_numerator),
            ("fee_b", n.fee_b_numerator),
            ("reward_0", n.reward_0_numerator),
            ("reward_1", n.reward_1_numerator),
            (
                "inner_vesting_liquidity",
                n.inner_vesting_liquidity_numerator,
            ),
        ] {
            assert!(
                value <= DENOMINATOR,
                "{fixture}: numerator {label} = {value} exceeds 1e9 — layout drift"
            );
        }
        // A split moves something, otherwise the instruction would be pointless.
        let a = &sp.amount_splits;
        assert!(
            a.unlocked_liquidity > 0
                || a.permanent_locked_liquidity > 0
                || a.vested_liquidity > 0
                || a.fee_a > 0
                || a.fee_b > 0,
            "{fixture}: split moved nothing at all"
        );
    }

    // The second fixture creates the receiving position in the same tx.
    let tx = load_fixture("damm_v2/split_position2.json");
    let extracted = extract_wire_events(&tx, &CP_AMM_PROGRAM_ID);
    assert!(
        extracted
            .events
            .iter()
            .any(|e| matches!(e.event, DammV2WireEvent::CreatePosition(_))),
        "expected the second position's creation in the same transaction"
    );
}

/// `EvtCreatePosition` rides along in the genesis transactions: decoded, sane, translated.
#[test]
fn decodes_create_position_from_genesis_fixtures() {
    for fixture in [
        "damm_v2/initialize_pool.json",
        "damm_v2/initialize_pool_2.json",
    ] {
        let tx = load_fixture(fixture);
        let extracted = extract_wire_events(&tx, &CP_AMM_PROGRAM_ID);
        assert!(
            extracted.failures.is_empty(),
            "{fixture}: failures: {:?}",
            extracted.failures
        );

        let create = extracted
            .events
            .iter()
            .find_map(|e| match &e.event {
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

/// `EvtClosePosition`, the same 4-pubkey shape as create: decoded, distinct fields, translated.
#[test]
fn decodes_close_position_fixture() {
    let tx = load_fixture("damm_v2/close_position.json");
    let extracted = extract_wire_events(&tx, &CP_AMM_PROGRAM_ID);
    assert!(extracted.failures.is_empty(), "{:?}", extracted.failures);

    let close = extracted
        .events
        .iter()
        .find_map(|e| match &e.event {
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

/// `EvtLockPosition` (4 pubkeys, u64×2, u128×2, u16): vesting numerics checked and round-tripped.
#[test]
fn decodes_lock_position_fixture() {
    let tx = load_fixture("damm_v2/lock_position.json");
    let extracted = extract_wire_events(&tx, &CP_AMM_PROGRAM_ID);
    assert!(extracted.failures.is_empty(), "{:?}", extracted.failures);

    let lock = extracted
        .events
        .iter()
        .find_map(|e| match &e.event {
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
    // Some liquidity is locked, at the cliff or over periods; a misaligned u128 fails this.
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

/// `EvtPermanentLockPosition`: the running total is at least this action's amount.
#[test]
fn decodes_permanent_lock_position_fixture() {
    let tx = load_fixture("damm_v2/permanent_lock_position.json");
    let extracted = extract_wire_events(&tx, &CP_AMM_PROGRAM_ID);
    assert!(
        extracted.failures.is_empty(),
        "unexpected extraction failures: {:?}",
        extracted.failures
    );

    let plock = extracted
        .events
        .iter()
        .find_map(|e| match &e.event {
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

/// `permanent_lock_position.json` also invokes cp-amm with a bare 8-byte discriminator, an
/// ordinary instruction. Mutation: testing the length before the `event_cpi` tag.
#[test]
fn an_ordinary_cp_amm_instruction_is_not_an_extraction_failure() {
    let tx = load_fixture("damm_v2/permanent_lock_position.json");
    assert!(
        cp_amm_payloads(&tx).any(|data| data.len() == 8),
        "the fixture no longer carries the 8-byte instruction this test is about"
    );

    let outcome = MeteoraDammV2::new().extract_events(&tx).expect("extract");
    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    assert!(
        outcome.events.iter().any(|e| matches!(
            e,
            DomainEvent::MeteoraDammV2(MeteoraDammV2Event::PermanentLockPosition(_))
        )),
        "no PermanentLockPosition domain event"
    );
}

/// The same instruction made a tagged, truncated event stays a counted failure.
/// Mutation: treating anything below 16 bytes as an ordinary instruction.
#[test]
fn a_tagged_but_truncated_payload_stays_an_extraction_failure() {
    let mut tx = load_fixture("damm_v2/permanent_lock_position.json");
    let before = extract_wire_events(&tx, &CP_AMM_PROGRAM_ID);

    let tagged = cp_amm_payloads(&tx)
        .nth(usize::from(before.events[0].event_index))
        .expect("the first event's payload");
    let truncated = [&tagged[..8], &[1, 2, 3]].concat();
    let ordinary = tx
        .inner_instructions
        .iter_mut()
        .find(|ix| ix.program_id == CP_AMM_PROGRAM_ID && ix.data.len() == 8)
        .expect("the fixture's 8-byte instruction");
    ordinary.data = truncated;

    let after = extract_wire_events(&tx, &CP_AMM_PROGRAM_ID);
    assert!(
        matches!(
            after.failures.as_slice(),
            [ExtractFailure::AnchorDecode {
                source: AnchorDecodeError::PayloadTooShort { got: 11, .. }
            }]
        ),
        "{:?}",
        after.failures
    );

    let outcome = MeteoraDammV2::new().extract_events(&tx).expect("extract");
    assert!(
        matches!(
            outcome.failures.as_slice(),
            [ExtractionFailure::AnchorDecode(_)]
        ),
        "{:?}",
        outcome.failures
    );
}

fn cp_amm_payloads(tx: &OnChainTransaction) -> impl Iterator<Item = &[u8]> {
    tx.inner_instructions
        .iter()
        .filter(|ix| ix.program_id == CP_AMM_PROGRAM_ID)
        .map(|ix| ix.data.as_slice())
}

/// `EvtClaimPositionFee` on a real claim: decoded, field-preserving translation.
#[test]
fn decodes_claim_position_fee_fixture() {
    let tx = load_fixture("damm_v2/claim_position_fee.json");
    let extracted = extract_wire_events(&tx, &CP_AMM_PROGRAM_ID);
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
        .find_map(|e| match &e.event {
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

/// `EvtClaimReward` on a real claim; `reward_index` disambiguates the reward stream.
#[test]
fn decodes_claim_reward_fixture() {
    let tx = load_fixture("damm_v2/claim_reward.json");
    let extracted = extract_wire_events(&tx, &CP_AMM_PROGRAM_ID);
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
        .find_map(|e| match &e.event {
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

/// `EvtUpdatePoolFees`, whose custom `BorshDeserialize` reads pool and operator, then captures the
/// trailing parameters verbatim into `params_raw`.
#[test]
fn decodes_update_pool_fees_fixture() {
    let tx = load_fixture("damm_v2/update_pool_fees.json");
    let extracted = extract_wire_events(&tx, &CP_AMM_PROGRAM_ID);

    assert!(
        extracted.failures.is_empty(),
        "unexpected extraction failures: {:?}",
        extracted.failures
    );

    let update = extracted
        .events
        .iter()
        .find_map(|e| match &e.event {
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
    // The trailing params blob, captured verbatim, is non-empty.
    assert!(
        !update.params_raw.is_empty(),
        "params_raw should be non-empty"
    );

    // End to end: pool, operator and the raw params blob are preserved.
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

/// `event_index` numbers the inner instructions to cp-amm, not the recognised events: one more
/// discriminator must not renumber stored events. The dropped `EvtSplitPosition2` leaves its hole.
#[test]
fn event_index_counts_dropped_payloads_too() {
    // `split_position.json`: one payload dropped, then the split at 1, not 0.
    // `split_position2.json`: two splits around a dropped payload, at 0 and 2.
    for (fixture, expected) in [
        ("damm_v2/split_position.json", vec![1u16]),
        ("damm_v2/split_position2.json", vec![0u16, 2u16]),
    ] {
        let tx = load_fixture(fixture);
        let extracted = extract_wire_events(&tx, &CP_AMM_PROGRAM_ID);

        let indices: Vec<u16> = extracted.events.iter().map(|e| e.event_index).collect();
        assert_eq!(
            indices, expected,
            "{fixture}: event_index must number the inner instructions to cp-amm, \
             including the deprecated EvtSplitPosition2 the extractor drops — \
             got {indices:?}, expected {expected:?}"
        );
    }
}
