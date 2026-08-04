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
    let tx = load_fixture("damm_v2/swap_double.json");

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

    for (i, indexed) in extracted.events.iter().enumerate() {
        assert!(
            matches!(indexed.event, DammV2WireEvent::Swap2(_)),
            "event {i} is not a Swap2: {indexed:?}"
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
    let tx = load_fixture("damm_v2/swap_double.json");

    let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);
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

/// `EvtLiquidityChange` — fixtures existed but had no decode test. Validate an
/// add-liquidity tx end-to-end: clean decode, `Add` kind, canonical sorted
/// mints, non-zero amounts/reserves/liquidity_delta, translation preserved.
#[test]
fn decodes_liquidity_add_fixtures() {
    for fixture in ["damm_v2/liquidity_add.json", "damm_v2/liquidity_add_2.json"] {
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

    let json = include_str!("fixtures/damm_v2/swap_via_router.json");
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
        "damm_v2/initialize_pool.json",
        "damm_v2/initialize_pool_2.json",
        "damm_v2/initialize_pool_3.json",
        "damm_v2/initialize_pool_4.json",
        "damm_v2/initialize_pool_5.json",
        "damm_v2/initialize_pool_6.json",
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
        // NOTE: cp-amm does NOT emit mints sorted by raw bytes — the event
        // preserves the program's native token_a/token_b designation. The
        // `initialize_pool_events` table stores this native order as-is; the
        // pool *registry* upsert re-sorts to the canonical convention shared
        // with the swap/liquidity tables (see persist_initialize_pool).
    }
}

/// The claim-protocol-fee fixture must decode cleanly to an
/// `EvtClaimProtocolFee` — the operator withdrawing Meteora's accrued protocol
/// fee share (the `emit_cpi!` variant, `ix_claim_protocol_fee`). Guards the
/// wire layout on real data.
#[test]
fn decodes_claim_protocol_fee_fixture() {
    let tx = load_fixture("damm_v2/claim_protocol_fee.json");
    let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);

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

/// The initialize-reward fixture must decode cleanly to an
/// `EvtInitializeReward` — an admin opening a farming reward slot on a pool.
///
/// This fixture is a real "farm launch" transaction: it opens slot 0 with a
/// 7-day window and funds it in the same transaction, so it also carries an
/// `EvtFundReward` (asserted separately). Every field is pinned to its
/// on-chain value: `funder` and `creator` are the same wallet here, so this
/// fixture cannot by itself discriminate their order in the borsh layout —
/// that comes from the cp-amm source.
#[test]
fn decodes_initialize_reward_fixture() {
    let tx = load_fixture("damm_v2/initialize_reward.json");
    let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);

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

/// The fund-reward fixtures must decode cleanly to an `EvtFundReward` — a
/// funder depositing rewards and (re)setting the slot's emission rate.
///
/// Two fixtures covering both regimes:
/// - `damm_v2/initialize_reward.json`: slot funded for the *first* time
///   (`pre_reward_rate == 0`), in the same transaction that opened it.
/// - `damm_v2/fund_reward.json`: a *re-fund* of a live slot
///   (`pre_reward_rate > 0`), where the undistributed remainder is carried
///   forward into the new window.
#[test]
fn decodes_fund_reward_fixtures() {
    // Fresh slot: opened and funded in one transaction.
    let tx = load_fixture("damm_v2/initialize_reward.json");
    let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);
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

    // The load-bearing assertion of this whole event: the rate is Q64.64.
    // On a fresh slot there is no carry-forward, so the program's rate must
    // equal (amount << 64) / duration exactly. A wrong scale assumption (or a
    // u128 field misread) breaks this; a mere non-zero check would not.
    let duration: u128 = 604_800;
    assert_eq!(
        fr.post_reward_rate,
        (u128::from(fr.amount) << 64) / duration,
        "post_reward_rate is not (amount << 64) / duration — Q64.64 scale drift"
    );

    // Re-fund of a live slot: carry-forward regime.
    let tx = load_fixture("damm_v2/fund_reward.json");
    let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);
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
    // Carry-forward: the new window distributes far more than this deposit
    // alone, because the remainder of the previous window was folded in.
    // total = post_rate * duration >> 64.
    let total = (fr.post_reward_rate * duration) >> 64;
    assert!(
        total > u128::from(fr.amount),
        "expected carry-forward: total ({total}) should exceed the deposit ({})",
        fr.amount
    );
}

/// The withdraw-ineligible-reward fixture must decode cleanly to an
/// `EvtWithdrawIneligibleReward` — the funder reclaiming rewards that accrued
/// while the pool had no eligible liquidity, so nobody could ever claim them.
///
/// Caveat on this fixture's strength: its `amount` is legitimately **zero**
/// (the instruction runs and emits even when there is nothing to reclaim), so
/// the amount field is pinned but not discriminating. The two pubkeys are.
#[test]
fn decodes_withdraw_ineligible_reward_fixture() {
    let tx = load_fixture("damm_v2/withdraw_ineligible_reward.json");
    let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);

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
    // Nothing was reclaimable in this transaction. Pinned rather than asserted
    // non-zero: a >0 check here would be a false guarantee.
    assert_eq!(wir.amount, 0);
}

/// A `zap_protocol_fee` transaction must yield **exactly one** event — and not
/// the one you might expect.
///
/// `zap_protocol_fee` is the operator-only instruction that harvests a pool's
/// accrued protocol fees in a long-tail token and immediately sells them toward
/// SOL/USDC. It is the documented blind spot of our indexing: it mutates pool
/// state through `pool.claim_protocol_fee()` but emits **no event at all**, so
/// the harvest itself is invisible to an `event_cpi` pipeline by construction.
/// This fixture turns that claim from a source-code reading into a fact checked
/// against real bytes.
///
/// The transaction's shape is what makes it interesting — two unrelated
/// top-level instructions that are easy to conflate:
///
/// ```text
/// [3] cp-amm  zap_protocol_fee   -> inner: one SPL transfer, ZERO event
/// [4] zapvX9M…  zap_out          -> Jupiter route
///                                     -> cp-amm swap (legacy v1)  => EvtSwap2
///                                     -> Whirlpool x2             (not ours)
/// ```
///
/// So the single `EvtSwap2` here belongs to instruction `[4]`: Jupiter routed
/// part of the sale through an *unrelated* cp-amm pool. Its pool key is
/// deliberately asserted below to be a different pool from the ones
/// `zap_protocol_fee` touches — seeing an event in this transaction does not
/// mean the fee harvest was captured.
///
/// Two decoder behaviours are pinned by the `unknown` / `failures` assertions:
/// the top-level `zap_protocol_fee` call and the nested legacy `swap` call are
/// both cp-amm instructions that are *not* `event_cpi` payloads, and must be
/// skipped silently rather than counted as unknown discriminators or failures.
#[test]
fn zap_protocol_fee_emits_no_event_of_its_own() {
    let tx = load_fixture("damm_v2/zap_protocol_fee.json");
    let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);

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
    // The pool Jupiter routed through while selling the harvested token — NOT a
    // pool that `zap_protocol_fee` harvested from.
    assert_eq!(
        swap.pool,
        pubkey!("4C47JJgDHztupNHeU9da7MroH676eXbPoUhEvx3FpfR3")
    );
    // An ordinary swap in every respect: real amounts, real post-swap reserves.
    assert_eq!(swap.included_transfer_fee_amount_in, 1_228_454_277_679);
    assert_eq!(swap.included_transfer_fee_amount_out, 1_615_941_369);
    assert!(swap.reserve_a_amount > 0 && swap.reserve_b_amount > 0);
}

/// The split-position fixtures must decode to `EvtSplitPosition3`, and the
/// deprecated `EvtSplitPosition2` that cp-amm emits alongside it must be
/// **dropped silently** — not counted as an unknown discriminator.
///
/// This is the double-counting guard. cp-amm emits both events unconditionally
/// on every split (the `#[allow(deprecated)]` block around the v2 emission is an
/// attribute scope, not a condition), and they describe the *same* split. v3 is
/// a strict superset, so v2 is recognised and discarded at extraction.
///
/// `damm_v2/split_position2.json` additionally carries an `EvtCreatePosition`:
/// a split needs a second position to split into, so the same transaction
/// creates it first. That event is indexed normally.
#[test]
fn decodes_split_position_fixtures_and_drops_the_deprecated_v2() {
    for fixture in [
        "damm_v2/split_position.json",
        "damm_v2/split_position2.json",
    ] {
        let tx = load_fixture(fixture);
        let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);

        assert!(
            extracted.failures.is_empty(),
            "{fixture}: {:?}",
            extracted.failures
        );
        // The whole point: EvtSplitPosition2 is emitted by every split. If it
        // ever shows up here, our deliberate drop stopped working and the
        // "unknown" metric is being polluted once per split.
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
        // Numerators are fractions over SPLIT_POSITION_DENOMINATOR = 1e9 and are
        // validated on-chain to stay within it. Garbage from a layout drift in
        // the trailing parameters struct would blow past this.
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
    let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);
    assert!(
        extracted
            .events
            .iter()
            .any(|e| matches!(e.event, DammV2WireEvent::CreatePosition(_))),
        "expected the second position's creation in the same transaction"
    );
}

/// `EvtCreatePosition` rides along in the genesis transactions (a pool is
/// created and its first position opened together), so the initialize_pool
/// fixtures double as real-data validation for it: clean decode, sane fields,
/// and a full translation into the domain event.
#[test]
fn decodes_create_position_from_genesis_fixtures() {
    for fixture in [
        "damm_v2/initialize_pool.json",
        "damm_v2/initialize_pool_2.json",
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

/// `EvtClosePosition` — same 4-pubkey shape as create. Validate against a real
/// close transaction: clean decode, sane distinct fields, translation preserved.
#[test]
fn decodes_close_position_fixture() {
    let tx = load_fixture("damm_v2/close_position.json");
    let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);
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

/// `EvtLockPosition` — non-trivial layout (4 pubkeys + u64×2 + u128×2 + u16).
/// A layout drift would scramble the pubkeys or fail borsh; the vesting
/// numerics are checked for coherence and round-tripped through the domain.
#[test]
fn decodes_lock_position_fixture() {
    let tx = load_fixture("damm_v2/lock_position.json");
    let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);
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
    let tx = load_fixture("damm_v2/permanent_lock_position.json");
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

/// `EvtClaimPositionFee` — validate against a real claim: clean decode, sane
/// pubkeys, and a full wire→domain translation preserving every field.
#[test]
fn decodes_claim_position_fee_fixture() {
    let tx = load_fixture("damm_v2/claim_position_fee.json");
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

/// `EvtClaimReward` — same shape of guard: decode + field-preserving translation
/// on a real reward claim. `reward_index` disambiguates the reward stream.
#[test]
fn decodes_claim_reward_fixture() {
    let tx = load_fixture("damm_v2/claim_reward.json");
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

/// Guard for the `EvtUpdatePoolFees` decode. Its `BorshDeserialize` is custom:
/// it reads the two leading pubkeys (pool, operator) and captures the trailing
/// `UpdatePoolFeesParameters` bytes verbatim into `params_raw` ("voie C"). A
/// clean decode here proves the discriminator matches and the prefix layout
/// (pool, operator) is correct on a real on-chain transaction.
#[test]
fn decodes_update_pool_fees_fixture() {
    let tx = load_fixture("damm_v2/update_pool_fees.json");
    let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);

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

/// `event_index` numbers the transaction's self-CPI payloads, **not** the
/// events we recognise — the property the unique key
/// `(signature, event_index, timestamp)` rests on.
///
/// The split fixtures prove it on real data rather than by construction:
/// cp-amm emits the deprecated `EvtSplitPosition2` alongside
/// `EvtSplitPosition3` on every split, and the extractor drops the v2
/// (`Dispatch::Ignored`). If indices were assigned over the *kept* events,
/// the surviving v3 would land at 0 and the dropped payload would leave no
/// trace. Numbering the payloads instead, it keeps the index it actually
/// occupied on chain.
///
/// Why this matters beyond tidiness: implement one more discriminator
/// tomorrow and a "kept events" numbering would renumber every event already
/// stored, so re-ingesting a transaction would insert duplicates instead of
/// conflicting. Stored indices must depend only on what the chain emitted.
#[test]
fn event_index_counts_dropped_payloads_too() {
    // `split_position.json`: one payload dropped, then the split — the kept
    // event sits at 1, not 0.
    // `split_position2.json`: two splits with a dropped payload *between*
    // them — the indices are 0 and 2, and the hole is the proof.
    for (fixture, expected) in [
        ("damm_v2/split_position.json", vec![1u16]),
        ("damm_v2/split_position2.json", vec![0u16, 2u16]),
    ] {
        let tx = load_fixture(fixture);
        let extracted = extract_wire_events(&tx, CP_AMM_PROGRAM_ID);

        let indices: Vec<u16> = extracted.events.iter().map(|e| e.event_index).collect();
        assert_eq!(
            indices, expected,
            "{fixture}: event_index must number the payloads cp-amm emitted, \
             including the deprecated EvtSplitPosition2 the extractor drops — \
             got {indices:?}, expected {expected:?}"
        );
    }
}
