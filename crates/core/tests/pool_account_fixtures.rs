//! The pool-account decoder, against **real mainnet accounts**.
//!
//! # Why real bytes are not optional here
//!
//! The decoder's unit tests build synthetic 1112-byte buffers using the very
//! constants they then verify. A wrong offset is invisible to them: the test
//! writes the field where the code will read it, so the two agree on a lie.
//!
//! That is not hypothetical. `partner_fee_percent` read byte 49 — cp-amm's
//! `padding_0` — for months. The neighbouring offsets (48, 50) were correct, so
//! two of three percents decoded fine, and the third came back 0 on all 971
//! pools: exactly what a plausible partner cut looks like. Only real bytes
//! settle that kind of question.
//!
//! # What these fixtures are
//!
//! One JSON file per pool under `fixtures/<protocol>/accounts/`, holding the raw
//! account as base64 plus its owner and capture date — **nothing decoded**. The
//! expectations live in `EXPECTED` / `EXPECTED_DLMM` below, in this file, so that
//! reviewing the test means reviewing what we claim the bytes mean.
//!
//! The eleven cp-amm pools cover every `BaseFeeMode` it defines and both values
//! of `has_dynamic_fee`. The rare ones were picked first because they are rare:
//! when this set was captured, mainnet held exactly **one** `rate_limiter` pool
//! and two market-cap linear ones among the 971 we had seen.
//!
//! The nine DLMM pools span `bin_step` 1..=400 and `base_factor` 0..=40 000,
//! including a zero-fee pool and pools with and without a dynamic fee. They were
//! found by resolving every account key in `fixtures/dlmm/*.json` and keeping
//! those owned by lb_clmm at 904 bytes — pools we have actually seen, not a
//! hand-picked list.
//!
//! # How the expectations were established
//!
//! Not by running our decoder and writing down what it said — that would restate
//! the bug we are guarding against. Three independent anchors:
//!
//! 1. **The realized fee rate, computed from swap-event amounts.** A completely
//!    separate data path: token amounts in event payloads, never account bytes.
//!    For `8Pm2kZ…` — constant fee, no dynamic fee — the fee decoded from byte 8
//!    is 4 bps and the rate realized over **49 639 swaps** is 4.0 bps. Two other
//!    constant pools agreed to within 0.3 bps.
//! 2. **The mints decode to known addresses.** `EPjFWdd5…` is USDC,
//!    `So1111…112` is wrapped SOL. A wrong offset yields arbitrary bytes, not a
//!    token anyone can name.
//! 3. **Consistency across eleven accounts.** A wrong `base_fee_mode` offset
//!    would have to land on a byte in `0..=4` eleven times, while the fee and
//!    both percents stayed plausible.
//!
//! # Recapturing
//!
//! ```text
//! curl -s -X POST "$SOLANA_RPC_HTTP" -H 'Content-Type: application/json' \
//!   -d '{"jsonrpc":"2.0","id":1,"method":"getMultipleAccounts",
//!        "params":[["<pool>", …], {"encoding":"base64"}]}'
//! ```
//!
//! A fixture is a **snapshot**. A program upgrade that moves a field will make
//! these fail, and that is the point: `PoolAccountRejection::Truncated` and the
//! discriminator check exist to surface exactly that, and a red test is the
//! right channel for it.

use std::path::PathBuf;

use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use std::str::FromStr;
use yog_core::{
    amm::damm_v2::BaseFeeKind,
    amm::dlmm::max_variable_fee_bps,
    application::decode_pool_account,
    domain::{PoolAccountProperties, Protocol},
};

/// What we claim one real account decodes to. `fee_bps` is a string so the
/// expected value reads as written (`"2.5"`, `"9900"`) rather than as a
/// `Decimal` construction.
struct Expected {
    pool: &'static str,
    kind: BaseFeeKind,
    fee_bps: &'static str,
    dynamic: bool,
    protocol_pct: u8,
    referral_pct: u8,
    mint_a: &'static str,
    mint_b: &'static str,
    /// The time-scheduler curve, as `(period_frequency, reduction_factor,
    /// activation_point)` — `None` for every fee shape that has no such curve.
    ///
    /// ⚠️ These three are the reason this expectation matters more than most.
    /// `BaseFeeInfo` is 32 bytes the modes reinterpret, so bytes 24 and 32 only
    /// mean "period frequency" and "reduction factor" for modes 0 and 1. Read
    /// unconditionally they are nonsense — and the nonsense is *plausible*
    /// enough to ship: mode 4 yields a period frequency of
    /// 13 722 280 043 814 587 382, mode 2 one of 42 520 176 273 600. `None`
    /// below is therefore an assertion in its own right, not an absence.
    scheduler: Option<(u64, u64, u64)>,
}

#[rustfmt::skip]
const EXPECTED: &[Expected] = &[
    Expected { pool: "28BDU1aghznh8t9Z1imygPU2DrLzw34FC5V9MHYb3HSA", kind: BaseFeeKind::SchedulerLinear, fee_bps: "5000", dynamic: true, protocol_pct: 20, referral_pct: 20, mint_a: "G95DFf3fjMqvTraw2T5EduHshNsrrNcaEA4QsD1upump", mint_b: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" , scheduler: Some((600, 3_194_444, 1_785_180_416)) },
    Expected { pool: "2hoh2jW3RLRLRrLagb6aWPL3txRMfWsgNstC4j2cdRhW", kind: BaseFeeKind::MarketCapSchedulerExponential, fee_bps: "300", dynamic: true, protocol_pct: 20, referral_pct: 20, mint_a: "RszpWXeCRFhKg2DV4MeCZYe1WEsZ6M5Wpgr5SyB1nat", mint_b: "So11111111111111111111111111111111111111112" , scheduler: None },
    Expected { pool: "4EqtnwiCSDJQJvVBrLh7pVdCxmo9rGKd66u4Esmq5Utt", kind: BaseFeeKind::MarketCapSchedulerLinear, fee_bps: "200", dynamic: true, protocol_pct: 20, referral_pct: 20, mint_a: "AoQGnPGXWHo9FfSVhPTmhJGvGXisEDwfaRPnDHHRpump", mint_b: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" , scheduler: None },
    Expected { pool: "59cbVFRS9GSYeMPVrNQtDyzGnaN8o3fyWZcPJxFuNZjD", kind: BaseFeeKind::SchedulerExponential, fee_bps: "1500", dynamic: false, protocol_pct: 20, referral_pct: 20, mint_a: "oEVufzrtcAvuefkbg2iQku9A6UbFh9f4V5kEiPARQEN", mint_b: "So11111111111111111111111111111111111111112" , scheduler: Some((1, 649, 1_783_696_139)) },
    Expected { pool: "7j7Qm6oeWZ2MFRve3kPWg1fE5cXYLDFPYe9982SjWrbC", kind: BaseFeeKind::RateLimiter, fee_bps: "400", dynamic: true, protocol_pct: 20, referral_pct: 20, mint_a: "PDqSePtjwXYaruFX7hdujV9wf4X7Z4fu5d2iVMCpump", mint_b: "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB" , scheduler: None },
    Expected { pool: "8Pm2kZpnxD3hoMmt4bjStX2Pw2Z9abpbHzZxMPqxPmie", kind: BaseFeeKind::Constant, fee_bps: "4", dynamic: false, protocol_pct: 20, referral_pct: 20, mint_a: "So11111111111111111111111111111111111111112", mint_b: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" , scheduler: None },
    Expected { pool: "CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j", kind: BaseFeeKind::Constant, fee_bps: "25", dynamic: true, protocol_pct: 20, referral_pct: 20, mint_a: "So11111111111111111111111111111111111111112", mint_b: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" , scheduler: None },
    Expected { pool: "FvAQ9jyDAqSGtTmLm5Mgpq3NhVhjfwEU8e6gUmtS1PqQ", kind: BaseFeeKind::SchedulerExponential, fee_bps: "9900", dynamic: true, protocol_pct: 20, referral_pct: 20, mint_a: "9xzmB67zWX8PJiGpQFbWuNBXTqyPpu2qV3mxQfhqUREV", mint_b: "UWUy7J86LUiBv5SjAUZ53LMGhtnqvbQ7QNSSkyupump" , scheduler: Some((1, 326, 1_783_799_458)) },
    Expected { pool: "FvXPAoRBA6QMWBMqjy1rCLuRkXDH3Q3zD6ZAv8yJ8U7j", kind: BaseFeeKind::MarketCapSchedulerExponential, fee_bps: "200", dynamic: false, protocol_pct: 20, referral_pct: 20, mint_a: "CCNN1WcqyhZntkSEb4fX6ARNT9TWoQNJ4SvoZxYzBAGS", mint_b: "So11111111111111111111111111111111111111112" , scheduler: None },
    Expected { pool: "KKyUyWncRfakBZh2M318BFfdR6332WWu1NePd9amQtj", kind: BaseFeeKind::MarketCapSchedulerLinear, fee_bps: "100", dynamic: true, protocol_pct: 20, referral_pct: 20, mint_a: "SV151D5pjygAKA8aJJcKzm4wFnRX5G92Fye94jQJk7g", mint_b: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" , scheduler: None },
    Expected { pool: "sZchbRCFoUcr3xzUhqtngzXCr2DUnvurd5hTx9NtXZB", kind: BaseFeeKind::SchedulerLinear, fee_bps: "1000", dynamic: false, protocol_pct: 20, referral_pct: 20, mint_a: "5je5ondjVJcHjWz2v4mLZn7PsQGr47XQFjFTfrtCu1ox", mint_b: "So11111111111111111111111111111111111111112" , scheduler: Some((3, 900_000, 1_778_597_368)) },
];

const CP_AMM_DIR: &str = "damm_v2";
const DLMM_DIR: &str = "dlmm";

/// What we claim one real `LbPair` account decodes to.
struct ExpectedDlmm {
    pool: &'static str,
    bin_step: u16,
    base_factor: u16,
    base_fee_power_factor: u8,
    /// `base_factor × bin_step × 10^power / 10_000`, as a string for the same
    /// reason as [`Expected::fee_bps`].
    fee_bps: &'static str,
    variable_fee_control: u32,
    max_volatility_accumulator: u32,
    protocol_share: u16,
    mint_x: &'static str,
    mint_y: &'static str,
}

#[rustfmt::skip]
const EXPECTED_DLMM: &[ExpectedDlmm] = &[
    ExpectedDlmm { pool: "HTvjzsfX3yU6BUodCjZ5vZkUrAxMDTrBs3CJaq43ashR", bin_step: 1, base_factor: 10_000, base_fee_power_factor: 0, fee_bps: "1", variable_fee_control: 2_000_000, max_volatility_accumulator: 100_000, protocol_share: 1000, mint_x: "So11111111111111111111111111111111111111112", mint_y: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" },
    ExpectedDlmm { pool: "DZ2vZJMLKt1cExzyFeyoGV3panTJufRFMiLXJKSa2mPP", bin_step: 1, base_factor: 10_000, base_fee_power_factor: 0, fee_bps: "1", variable_fee_control: 0, max_volatility_accumulator: 0, protocol_share: 1000, mint_x: "JuprjznTrTSp2UFa3ZBUFgwdAmtZCq4MQCwysN55USD", mint_y: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" },
    ExpectedDlmm { pool: "K8yYaCkTPBoTNkmejTtX2A8HinSqzfsf3gZJKWHs9yH",  bin_step: 1, base_factor: 0, base_fee_power_factor: 0, fee_bps: "0", variable_fee_control: 0, max_volatility_accumulator: 0, protocol_share: 500, mint_x: "2U3HtjyWFyJ47WX8MiWbiZYrgL1Qi1rwEwkQyuLEMUTa", mint_y: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" },
    ExpectedDlmm { pool: "Go3Fuj12qq2FCAbG2P9o72L38JENfcxhw8VFhbfHtP1o", bin_step: 2, base_factor: 5_000, base_fee_power_factor: 0, fee_bps: "1", variable_fee_control: 50_000, max_volatility_accumulator: 150_000, protocol_share: 2000, mint_x: "EBvpdu9qTNaVRfve6uadzXKjNPyhN33Kj4GF6a6WQNvd", mint_y: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" },
    ExpectedDlmm { pool: "8pZhpZrGtaLksLq1m1yZ333TChxmRAW7K4DxnfyetsUj", bin_step: 100, base_factor: 500, base_fee_power_factor: 0, fee_bps: "5", variable_fee_control: 7_500, max_volatility_accumulator: 150_000, protocol_share: 1000, mint_x: "Gihwz9Dj89Lt9bByouPEdD3bT37y2hamwDdxMuPWAVz", mint_y: "So11111111111111111111111111111111111111112" },
    ExpectedDlmm { pool: "JCYMX9Nx7DTUdguptRR5LLSc62MEbNmFYsbT5R9yCDGy", bin_step: 50, base_factor: 5_000, base_fee_power_factor: 0, fee_bps: "25", variable_fee_control: 10_000, max_volatility_accumulator: 250_000, protocol_share: 1000, mint_x: "AuQaustGiaqxRvj2gtCdrd22PBzTn8kM3kEPEkZCtuDw", mint_y: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" },
    ExpectedDlmm { pool: "8nXD5VHhcKpehJKLgbMAxQ2myCqbYZiCk77K7J4K1fFz", bin_step: 400, base_factor: 1_250, base_fee_power_factor: 0, fee_bps: "50", variable_fee_control: 50_000, max_volatility_accumulator: 150_000, protocol_share: 2000, mint_x: "GEy7ycaeocLsQWJftjTL6xF1UqmruMBBBh5PqBkXvrSv", mint_y: "Xsa62P5mvPszXL1krVUnU5ar38bBSVcWAB6fmPCo5Zu" },
    ExpectedDlmm { pool: "8KvuP878qiUxsc5P8X6mmjcNU5hzj7LBXbRhDSca5ej9", bin_step: 25, base_factor: 40_000, base_fee_power_factor: 0, fee_bps: "100", variable_fee_control: 50_000, max_volatility_accumulator: 150_000, protocol_share: 2000, mint_x: "EBnaKqUAk6ut1nse3R19CHCfY4jHD5SWdxB1UxyuYsRu", mint_y: "So11111111111111111111111111111111111111112" },
    ExpectedDlmm { pool: "7t1sXtcsSJ8Yg8UKgZp7mjv3HQQ9KW5v8FJ9cfU34GT3", bin_step: 200, base_factor: 10_000, base_fee_power_factor: 0, fee_bps: "200", variable_fee_control: 7_500, max_volatility_accumulator: 150_000, protocol_share: 1000, mint_x: "6zgbwgsiQkNP6EXHx7gEXqgKGK4RPpFWvW2HFwi7pump", mint_y: "So11111111111111111111111111111111111111112" },
];

/// A captured account: raw bytes and provenance, nothing interpreted.
///
/// `captured_at` lives in the JSON but not here — it is provenance for whoever
/// reads the file, and serde ignores what the struct does not name.
#[derive(serde::Deserialize)]
struct AccountFixture {
    /// Restated inside the file so a mismatch with the file name is caught —
    /// a fixture copied from another pool and renamed would otherwise assert
    /// one pool's expectations against another's bytes.
    pool_address: String,
    owner: String,
    data_base64: String,
}

fn load(protocol_dir: &str, pool: &str) -> AccountFixture {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(format!("tests/fixtures/{protocol_dir}/accounts"));
    path.push(format!("{pool}.json"));

    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("failed to parse fixture {}: {e}", path.display()))
}

/// Minimal base64 decode — the RPC's encoding, not the chain's, so `core` has no
/// dependency for it and neither does this test.
fn base64_decode(s: &str) -> Vec<u8> {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let (mut out, mut acc, mut bits) = (Vec::new(), 0u32, 0u8);
    for c in s.bytes().filter(|c| *c != b'=') {
        let v = A.iter().position(|a| *a == c).expect("invalid base64") as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

/// Every field, on every captured account.
///
/// One test rather than eleven: a layout drift breaks all of them at once, and
/// the pool address in each assertion is enough to place a genuinely
/// pool-specific failure.
#[test]
fn decodes_real_mainnet_accounts() {
    for e in EXPECTED {
        let fixture = load(CP_AMM_DIR, e.pool);
        let data = base64_decode(&fixture.data_base64);

        assert_eq!(
            fixture.pool_address, e.pool,
            "fixture file {}.json holds another pool's bytes",
            e.pool
        );
        assert_eq!(
            fixture.owner, "cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG",
            "{}: fixture is not a cp-amm account",
            e.pool
        );
        assert_eq!(data.len(), 1112, "{}: unexpected account length", e.pool);

        let owner = Pubkey::from_str(&fixture.owner).expect("owner");
        let decoded = decode_pool_account(&owner, &data)
            .unwrap_or_else(|err| panic!("{}: real account rejected: {err}", e.pool));

        assert_eq!(decoded.protocol(), Protocol::MeteoraDammV2, "{}", e.pool);

        // The registry half — the values every protocol has.
        assert_eq!(
            decoded.registry.fee_bps,
            Decimal::from_str(e.fee_bps).unwrap(),
            "{}: base fee",
            e.pool
        );
        assert_eq!(
            decoded.registry.token_a_mint,
            Pubkey::from_str(e.mint_a).unwrap(),
            "{}: token A",
            e.pool
        );
        assert_eq!(
            decoded.registry.token_b_mint,
            Pubkey::from_str(e.mint_b).unwrap(),
            "{}: token B",
            e.pool
        );

        // The cp-amm half.
        let PoolAccountProperties::MeteoraDammV2(props) = decoded.properties else {
            panic!("{}: decoded as another protocol", e.pool)
        };
        assert_eq!(
            props.base_fee_kind,
            Some(e.kind),
            "{}: fee shape — a mode this build cannot map yields None",
            e.pool
        );
        assert_eq!(props.has_dynamic_fee, e.dynamic, "{}: dynamic fee", e.pool);

        // The decay curve, and — just as load-bearing — its absence. Only the
        // two time-scheduler modes may carry one; every other mode reading a
        // `Some` here would mean the decoder took bytes 24/32 at face value on a
        // layout that puts other fields there.
        let scheduler = props
            .fee_scheduler
            .map(|s| (s.period_frequency, s.reduction_factor, s.activation_point));
        assert_eq!(scheduler, e.scheduler, "{}: fee scheduler curve", e.pool);
        if let Some(s) = props.fee_scheduler {
            assert_eq!(
                s.cliff_fee_numerator,
                u64::try_from((decoded.registry.fee_bps * Decimal::from(100_000)).trunc())
                    .expect("fee numerator fits u64"),
                "{}: the scheduler's cliff must be the very numerator fee_bps is derived from",
                e.pool
            );
            assert_eq!(
                s.activation_type, 1,
                "{}: captured accounts are all timestamp-activated",
                e.pool
            );
        }
        assert_eq!(
            props.protocol_fee_percent, e.protocol_pct,
            "{}: protocol cut",
            e.pool
        );
        assert_eq!(
            props.referral_fee_percent, e.referral_pct,
            "{}: referral cut",
            e.pool
        );
    }
}

// ── DLMM `LbPair` ───────────────────────────────────────────────────

/// Every field, on every captured `LbPair`.
///
/// The synthetic unit tests in `decoder_tests.rs` cannot settle the DLMM offsets
/// either — same circularity, same remedy. Two of the three anchors described at
/// the top of this file carry over directly:
///
/// - **The mints decode to nameable tokens.** USDC, wrapped SOL, JupUSD and
///   pump.fun mints across nine accounts. A wrong offset yields arbitrary bytes.
/// - **The fees land on real tiers.** 0, 1, 5, 25, 50, 100 and 200 bps — the
///   values Meteora publishes. A wrong `base_factor` or `bin_step` offset would
///   have to produce a plausible fee tier nine times running.
#[test]
fn decodes_real_mainnet_lb_pairs() {
    for e in EXPECTED_DLMM {
        let fixture = load(DLMM_DIR, e.pool);
        let data = base64_decode(&fixture.data_base64);

        assert_eq!(
            fixture.pool_address, e.pool,
            "fixture file {}.json holds another pool's bytes",
            e.pool
        );
        assert_eq!(
            fixture.owner, "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo",
            "{}: fixture is not a DLMM account",
            e.pool
        );
        assert_eq!(data.len(), 904, "{}: unexpected account length", e.pool);

        let owner = Pubkey::from_str(&fixture.owner).expect("owner");
        let decoded = decode_pool_account(&owner, &data)
            .unwrap_or_else(|err| panic!("{}: real account rejected: {err}", e.pool));

        assert_eq!(decoded.protocol(), Protocol::MeteoraDlmm, "{}", e.pool);

        // The registry half — the values every protocol has.
        assert_eq!(
            decoded.registry.fee_bps,
            Decimal::from_str(e.fee_bps).unwrap(),
            "{}: base fee",
            e.pool
        );
        assert_eq!(
            decoded.registry.token_a_mint,
            Pubkey::from_str(e.mint_x).unwrap(),
            "{}: token X",
            e.pool
        );
        assert_eq!(
            decoded.registry.token_b_mint,
            Pubkey::from_str(e.mint_y).unwrap(),
            "{}: token Y",
            e.pool
        );

        // The DLMM half.
        let PoolAccountProperties::MeteoraDlmm(props) = decoded.properties else {
            panic!("{}: decoded as another protocol", e.pool)
        };
        assert_eq!(props.bin_step, e.bin_step, "{}: bin step", e.pool);
        assert_eq!(props.base_factor, e.base_factor, "{}: base factor", e.pool);
        assert_eq!(
            props.base_fee_power_factor, e.base_fee_power_factor,
            "{}: base fee power factor",
            e.pool
        );
        assert_eq!(
            props.variable_fee_control, e.variable_fee_control,
            "{}: variable fee control",
            e.pool
        );
        assert_eq!(
            props.max_volatility_accumulator, e.max_volatility_accumulator,
            "{}: max volatility accumulator",
            e.pool
        );
        assert_eq!(
            props.protocol_share, e.protocol_share,
            "{}: protocol share",
            e.pool
        );
    }
}

/// **What `fee_bps` does not say.** The base fee is the floor; each pool caps
/// its own volatility-driven part, and on these very accounts that ceiling runs
/// from ×1 to ×7 the floor.
///
/// Asserted here rather than only documented, because the claim is quantitative
/// and made in three places (`amm::dlmm`, the api README, the PR). The inputs
/// come from the decoder, so a layout drift breaks this too.
///
/// The worst case is `min(base + variable, 1000)` — the chain caps the sum, so
/// the two bounds do not simply add.
#[test]
fn the_fee_floor_understates_what_a_dlmm_pool_can_charge() {
    // pool → (max variable bps, worst case bps). Both derived, never hand-typed.
    let expected: &[(&str, &str, &str)] = &[
        ("DZ2vZJMLKt1cExzyFeyoGV3panTJufRFMiLXJKSa2mPP", "0", "1"),
        ("HTvjzsfX3yU6BUodCjZ5vZkUrAxMDTrBs3CJaq43ashR", "2", "3"),
        (
            "8KvuP878qiUxsc5P8X6mmjcNU5hzj7LBXbRhDSca5ej9",
            "70.3125",
            "170.3125",
        ),
        ("7t1sXtcsSJ8Yg8UKgZp7mjv3HQQ9KW5v8FJ9cfU34GT3", "675", "875"),
        (
            "JCYMX9Nx7DTUdguptRR5LLSc62MEbNmFYsbT5R9yCDGy",
            "156.25",
            "181.25",
        ),
    ];

    for (pool, max_variable, worst_case) in expected {
        let e = EXPECTED_DLMM
            .iter()
            .find(|e| e.pool == *pool)
            .unwrap_or_else(|| panic!("{pool} is no longer a captured fixture"));

        let variable = max_variable_fee_bps(
            e.variable_fee_control,
            e.max_volatility_accumulator,
            e.bin_step,
        );
        assert_eq!(
            variable,
            Decimal::from_str(max_variable).unwrap(),
            "{pool}: variable-fee ceiling",
        );

        let base = Decimal::from_str(e.fee_bps).unwrap();
        assert_eq!(
            (base + variable).min(Decimal::from(1_000)),
            Decimal::from_str(worst_case).unwrap(),
            "{pool}: worst case a swapper can pay",
        );
    }
}

/// The point of the test above, stated as an invariant rather than a table: at
/// least one captured pool charges several times its advertised tier, and at
/// least one charges exactly it. A capture that lost either end would leave the
/// "not interchangeable" claim untested in one direction.
#[test]
fn the_fixtures_span_both_ends_of_the_fee_gap() {
    let ratios: Vec<Decimal> = EXPECTED_DLMM
        .iter()
        .filter(|e| e.base_factor != 0)
        .map(|e| {
            let base = Decimal::from_str(e.fee_bps).unwrap();
            let variable = max_variable_fee_bps(
                e.variable_fee_control,
                e.max_volatility_accumulator,
                e.bin_step,
            );
            (base + variable) / base
        })
        .collect();

    assert!(
        ratios.contains(&Decimal::ONE),
        "no pool whose fee_bps is the whole story (none without a dynamic fee)"
    );
    assert!(
        ratios.iter().any(|r| *r >= Decimal::from(5)),
        "no pool charging several times its tier — the gap claim is untested"
    );
}

/// The captured set spans the fee inputs widely enough that an offset landing on
/// a neighbouring field could not survive it.
///
/// `bin_step` runs 1..=400 and `base_factor` 0..=40 000, so neither can be a
/// constant the decoder happens to read from the wrong place — and both extremes
/// of the dynamic-fee magnitude are present (`variable_fee_control = 0`, which is
/// how DLMM expresses "no dynamic fee", and a live 2 000 000).
#[test]
fn the_lb_pair_fixtures_span_the_fee_inputs() {
    let bin_steps: Vec<u16> = EXPECTED_DLMM.iter().map(|e| e.bin_step).collect();
    assert!(bin_steps.contains(&1), "no minimum-bin-step pool");
    assert!(bin_steps.iter().any(|s| *s >= 200), "no wide-bin-step pool");

    assert!(
        EXPECTED_DLMM.iter().any(|e| e.base_factor == 0),
        "no zero-base-fee pool"
    );
    assert!(
        EXPECTED_DLMM.iter().any(|e| e.base_factor >= 40_000),
        "no high-base-factor pool"
    );
    assert!(
        EXPECTED_DLMM.iter().any(|e| e.variable_fee_control == 0),
        "no pool without a dynamic fee"
    );
    assert!(
        EXPECTED_DLMM.iter().any(|e| e.variable_fee_control > 0),
        "no pool with a dynamic fee"
    );
}

/// **A gap this suite cannot close, stated rather than hidden.**
///
/// `base_fee_power_factor` is 0 on all nine captured pools — and on all 42
/// `LbPair` accounts reachable from the DLMM transaction fixtures. So the byte
/// at offset 34 is *consistent with* being the power factor, but no real account
/// exercises it: every fixture would decode identically if the field were
/// ignored entirely.
///
/// That is exactly the shape of the `partner_fee_percent` bug — a field always 0
/// in the wild. The difference is that lb_clmm names this one and Meteora's
/// formula documents it, so it is a real field with no live user, not a padding
/// byte read by mistake. [`yog_core::amm::dlmm::base_fee_bps`] is unit-tested on
/// non-zero values; only the *offset* is unwitnessed.
///
/// Capture a pool with a non-zero power factor if one ever appears, and this
/// test becomes redundant.
#[test]
fn the_power_factor_offset_is_not_witnessed_by_any_fixture() {
    assert!(
        EXPECTED_DLMM.iter().all(|e| e.base_fee_power_factor == 0),
        "a fixture now exercises base_fee_power_factor — assert its offset \
         directly and delete this test"
    );
}

/// Byte 49 is `padding_0`, and every real account confirms it: zero on all
/// eleven, which is what padding looks like and what a partner cut would only
/// coincidentally look like.
///
/// The decoder must have no constant for it — this asserts the observation that
/// made migration 037's case, on the data that settles it.
#[test]
fn byte_49_is_padding_on_every_real_account() {
    for e in EXPECTED {
        let data = base64_decode(&load(CP_AMM_DIR, e.pool).data_base64);
        assert_eq!(
            data[49], 0,
            "{}: byte 49 is cp-amm's padding_0, not a fee",
            e.pool
        );
    }
}

/// The captured set covers every `BaseFeeMode` cp-amm defines, and both values
/// of the dynamic-fee flag.
///
/// Asserted rather than trusted: the rare modes are one pool each on mainnet, so
/// a future recapture that quietly drops one would leave the market-cap
/// schedulers — added without any real data ever showing them — untested again.
#[test]
fn the_fixtures_cover_every_fee_mode() {
    for kind in [
        BaseFeeKind::Constant,
        BaseFeeKind::SchedulerLinear,
        BaseFeeKind::SchedulerExponential,
        BaseFeeKind::RateLimiter,
        BaseFeeKind::MarketCapSchedulerLinear,
        BaseFeeKind::MarketCapSchedulerExponential,
    ] {
        assert!(
            EXPECTED.iter().any(|e| e.kind == kind),
            "no captured account covers {kind:?}"
        );
    }
    assert!(
        EXPECTED.iter().any(|e| e.dynamic),
        "no pool with a dynamic fee"
    );
    assert!(
        EXPECTED.iter().any(|e| !e.dynamic),
        "no pool without a dynamic fee"
    );
}
