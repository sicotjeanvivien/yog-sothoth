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
//! One JSON file per pool under `fixtures/damm_v2/accounts/`, holding the raw
//! account as base64 plus its owner and capture date — **nothing decoded**. The
//! expectations live in `EXPECTED` below, in this file, so that reviewing the
//! test means reviewing what we claim the bytes mean.
//!
//! The eleven pools cover every `BaseFeeMode` cp-amm defines and both values of
//! `has_dynamic_fee`. The rare ones were picked first because they are rare:
//! when this set was captured, mainnet held exactly **one** `rate_limiter` pool
//! and two market-cap linear ones among the 971 we had seen.
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
}

#[rustfmt::skip]
const EXPECTED: &[Expected] = &[
    Expected { pool: "28BDU1aghznh8t9Z1imygPU2DrLzw34FC5V9MHYb3HSA", kind: BaseFeeKind::SchedulerLinear, fee_bps: "5000", dynamic: true, protocol_pct: 20, referral_pct: 20, mint_a: "G95DFf3fjMqvTraw2T5EduHshNsrrNcaEA4QsD1upump", mint_b: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" },
    Expected { pool: "2hoh2jW3RLRLRrLagb6aWPL3txRMfWsgNstC4j2cdRhW", kind: BaseFeeKind::MarketCapSchedulerExponential, fee_bps: "300", dynamic: true, protocol_pct: 20, referral_pct: 20, mint_a: "RszpWXeCRFhKg2DV4MeCZYe1WEsZ6M5Wpgr5SyB1nat", mint_b: "So11111111111111111111111111111111111111112" },
    Expected { pool: "4EqtnwiCSDJQJvVBrLh7pVdCxmo9rGKd66u4Esmq5Utt", kind: BaseFeeKind::MarketCapSchedulerLinear, fee_bps: "200", dynamic: true, protocol_pct: 20, referral_pct: 20, mint_a: "AoQGnPGXWHo9FfSVhPTmhJGvGXisEDwfaRPnDHHRpump", mint_b: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" },
    Expected { pool: "59cbVFRS9GSYeMPVrNQtDyzGnaN8o3fyWZcPJxFuNZjD", kind: BaseFeeKind::SchedulerExponential, fee_bps: "1500", dynamic: false, protocol_pct: 20, referral_pct: 20, mint_a: "oEVufzrtcAvuefkbg2iQku9A6UbFh9f4V5kEiPARQEN", mint_b: "So11111111111111111111111111111111111111112" },
    Expected { pool: "7j7Qm6oeWZ2MFRve3kPWg1fE5cXYLDFPYe9982SjWrbC", kind: BaseFeeKind::RateLimiter, fee_bps: "400", dynamic: true, protocol_pct: 20, referral_pct: 20, mint_a: "PDqSePtjwXYaruFX7hdujV9wf4X7Z4fu5d2iVMCpump", mint_b: "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB" },
    Expected { pool: "8Pm2kZpnxD3hoMmt4bjStX2Pw2Z9abpbHzZxMPqxPmie", kind: BaseFeeKind::Constant, fee_bps: "4", dynamic: false, protocol_pct: 20, referral_pct: 20, mint_a: "So11111111111111111111111111111111111111112", mint_b: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" },
    Expected { pool: "CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j", kind: BaseFeeKind::Constant, fee_bps: "25", dynamic: true, protocol_pct: 20, referral_pct: 20, mint_a: "So11111111111111111111111111111111111111112", mint_b: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" },
    Expected { pool: "FvAQ9jyDAqSGtTmLm5Mgpq3NhVhjfwEU8e6gUmtS1PqQ", kind: BaseFeeKind::SchedulerExponential, fee_bps: "9900", dynamic: true, protocol_pct: 20, referral_pct: 20, mint_a: "9xzmB67zWX8PJiGpQFbWuNBXTqyPpu2qV3mxQfhqUREV", mint_b: "UWUy7J86LUiBv5SjAUZ53LMGhtnqvbQ7QNSSkyupump" },
    Expected { pool: "FvXPAoRBA6QMWBMqjy1rCLuRkXDH3Q3zD6ZAv8yJ8U7j", kind: BaseFeeKind::MarketCapSchedulerExponential, fee_bps: "200", dynamic: false, protocol_pct: 20, referral_pct: 20, mint_a: "CCNN1WcqyhZntkSEb4fX6ARNT9TWoQNJ4SvoZxYzBAGS", mint_b: "So11111111111111111111111111111111111111112" },
    Expected { pool: "KKyUyWncRfakBZh2M318BFfdR6332WWu1NePd9amQtj", kind: BaseFeeKind::MarketCapSchedulerLinear, fee_bps: "100", dynamic: true, protocol_pct: 20, referral_pct: 20, mint_a: "SV151D5pjygAKA8aJJcKzm4wFnRX5G92Fye94jQJk7g", mint_b: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" },
    Expected { pool: "sZchbRCFoUcr3xzUhqtngzXCr2DUnvurd5hTx9NtXZB", kind: BaseFeeKind::SchedulerLinear, fee_bps: "1000", dynamic: false, protocol_pct: 20, referral_pct: 20, mint_a: "5je5ondjVJcHjWz2v4mLZn7PsQGr47XQFjFTfrtCu1ox", mint_b: "So11111111111111111111111111111111111111112" },
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

fn load(pool: &str) -> AccountFixture {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/fixtures/damm_v2/accounts");
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
        let fixture = load(e.pool);
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
        let PoolAccountProperties::MeteoraDammV2(props) = decoded.properties;
        assert_eq!(
            props.base_fee_kind,
            Some(e.kind),
            "{}: fee shape — a mode this build cannot map yields None",
            e.pool
        );
        assert_eq!(props.has_dynamic_fee, e.dynamic, "{}: dynamic fee", e.pool);
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

/// Byte 49 is `padding_0`, and every real account confirms it: zero on all
/// eleven, which is what padding looks like and what a partner cut would only
/// coincidentally look like.
///
/// The decoder must have no constant for it — this asserts the observation that
/// made migration 037's case, on the data that settles it.
#[test]
fn byte_49_is_padding_on_every_real_account() {
    for e in EXPECTED {
        let data = base64_decode(&load(e.pool).data_base64);
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
