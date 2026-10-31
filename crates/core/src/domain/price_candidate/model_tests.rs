//! When [`PriceCandidate`] is asked, and which class a mint falls in.

use chrono::DateTime;

use super::*;
use crate::domain::{KeptPrices, PRICE_MAX_AGE_LATEST, PriceProvider, TokenPrice};

fn mint_from(first_byte: u8) -> Pubkey {
    let mut bytes = [7u8; 32];
    bytes[0] = first_byte;
    Pubkey::new_from_array(bytes)
}

fn turns(candidate: &PriceCandidate, ticks: u64) -> Vec<u64> {
    (0..ticks).filter(|tick| candidate.is_turn(*tick)).collect()
}

/// Against the rule on the wrong order, and a class left at the wrong one.
#[test]
fn never_priced_comes_before_low_value_and_regular_is_what_remains() {
    use PriceCandidateClass::{LowValue, NeverPriced, Regular};

    assert_eq!(PriceCandidateClass::of(true, true), NeverPriced);
    assert_eq!(PriceCandidateClass::of(true, false), NeverPriced);
    assert_eq!(PriceCandidateClass::of(false, true), LowValue);
    assert_eq!(PriceCandidateClass::of(false, false), Regular);
}

/// Against an every-ticks of 1 read as "never", or a slot compared without its modulo.
#[test]
fn a_regular_mint_is_asked_at_every_tick() {
    let candidate = PriceCandidate {
        mint: mint_from(200),
        class: PriceCandidateClass::Regular,
    };

    assert_eq!(turns(&candidate, 5), vec![0, 1, 2, 3, 4]);
}

/// Against a slot compared without its modulo, which is never reached, and a cycle of another length.
#[test]
fn a_mint_is_asked_exactly_once_per_cycle_of_its_class() {
    for (class, every) in [
        (PriceCandidateClass::LowValue, LOW_VALUE_EVERY_TICKS),
        (PriceCandidateClass::NeverPriced, NEVER_PRICED_EVERY_TICKS),
    ] {
        for first_byte in [0, 1, 2, 104, 105, 200, 255] {
            let candidate = PriceCandidate {
                mint: mint_from(first_byte),
                class,
            };

            let cycle = turns(&candidate, every);
            assert_eq!(
                cycle.len(),
                1,
                "{class:?} from byte {first_byte}: {cycle:?}"
            );
            assert!(candidate.is_turn(cycle[0] + every), "{class:?} comes back");
        }
    }
}

/// Against a constant slot, which would ask a whole class at one tick.
#[test]
fn a_class_is_spread_over_its_ticks() {
    let mut asked = [0; LOW_VALUE_EVERY_TICKS as usize];
    for first_byte in 0..=255 {
        let candidate = PriceCandidate {
            mint: mint_from(first_byte),
            class: PriceCandidateClass::LowValue,
        };
        asked[turns(&candidate, LOW_VALUE_EVERY_TICKS)[0] as usize] += 1;
    }

    assert!(asked.iter().all(|count| *count >= 85), "{asked:?}");
}

/// Against a slot read from the last bytes, which a vanity suffix makes the same for all.
#[test]
fn the_slot_ignores_a_shared_suffix() {
    let mut bytes = [0u8; 32];
    bytes[28..].copy_from_slice(b"pump");
    let first = Pubkey::new_from_array(bytes);
    bytes[0] = 1;
    let second = Pubkey::new_from_array(bytes);

    assert_ne!(address_slot(&first), address_slot(&second));
}

/// Against the floor back at nine and a half minutes, or a low-value mint asked one tick in two:
/// a motionless price would wait for a later turn, past the staleness bound.
#[test]
fn a_motionless_low_value_price_stays_valid_at_any_tick_length_the_cadence_allows() {
    let mint = mint_from(1);
    let price_at = |fetched_at| TokenPrice {
        mint,
        price_usd: Decimal::ONE,
        price_provider: PriceProvider::Jupiter,
        confidence: None,
        fetched_at,
    };
    let t0 = DateTime::from_timestamp(1_790_000_000, 0).expect("valid timestamp");

    for tick_secs in (165..=295).step_by(5) {
        let mut kept = KeptPrices::new(core::time::Duration::from_secs(30));
        kept.record(&[price_at(t0)]);

        let turn = Duration::seconds(tick_secs * LOW_VALUE_EVERY_TICKS as i64);
        let mut when = t0 + turn;
        while !kept.worth_keeping(&price_at(when)) {
            when += turn;
        }

        assert!(
            when - t0 < PRICE_MAX_AGE_LATEST,
            "ticks of {tick_secs} s: rewritten after {} s",
            (when - t0).num_seconds()
        );
    }
}
