//! Live integration tests for the DAMM v2 wire event extractor.
//!
//! Each test loads a real Solana transaction (saved as JSON in
//! `tests/fixtures/`) and asserts that the extractor produces the
//! expected wire events.
//!
//! Fixtures are dumped via `solana confirm -v <signature> --output json`
//! against mainnet — they capture the exact shape the RPC returns, so
//! these tests double as regression guards if the JSON schema ever drifts.

use solana_pubkey::pubkey;
use solana_transaction_status_client_types::EncodedConfirmedTransactionWithStatusMeta;
use std::path::PathBuf;

use yog_core::{
    application::extraction::{
        EventExtractor, MeteoraDammV2,
        meteora::damm_v2::{events::DammV2WireEvent, extractor::extract_wire_events},
    },
    domain::{DomainEvent, MeteoraDammV2Event},
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

    // Mints: the relevant transferChecked group is (USDC, SOL, SOL_dust).
    // We take the first 2 unique mints in order, sorted by raw bytes.
    // SOL = So11... < USDC = EPjFW... (raw bytes), so token_a = SOL, token_b = USDC.
    assert_eq!(
        swap.token_a_mint,
        pubkey!("So11111111111111111111111111111111111111112"),
    );
    assert_eq!(
        swap.token_b_mint,
        pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"),
    );

    // The amounts come from the EvtSwap2 wire fields, mapped via trade_direction.
    // We don't hard-code them here — the EvtSwap2 borsh payload is what drives
    // the values. We just sanity-check they're nonzero.
    assert!(swap.amount_a > 0, "amount_a should be nonzero");
    assert!(swap.amount_b > 0, "amount_b should be nonzero");
    assert!(swap.reserve_a_after > 0);
    assert!(swap.reserve_b_after > 0);
}
