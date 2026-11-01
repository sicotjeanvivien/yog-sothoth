//! The decoder on fabricated payloads: the type that comes out, and how an unreadable one counts.

use chrono::DateTime;
use solana_signature::Signature;

use super::*;
use crate::application::extraction::{
    EventExtractor, ExtractionFailure, InnerInstructionPayload, MeteoraDammV2,
    anchor_event::EVENT_IX_TAG,
};
use crate::domain::{DomainEvent, MeteoraDammV2Event, Protocol, TransactionPosition};

fn program() -> Pubkey {
    Protocol::MeteoraDammV2.program_id()
}

fn pk(byte: u8) -> Pubkey {
    Pubkey::new_from_array([byte; 32])
}

fn event_cpi(discriminator: [u8; DISCRIMINATOR_LEN], body: &[u8]) -> Vec<u8> {
    [&EVENT_IX_TAG[..], &discriminator[..], body].concat()
}

fn transaction(payloads: Vec<Vec<u8>>) -> OnChainTransaction {
    OnChainTransaction {
        position: TransactionPosition {
            signature: Signature::from([7u8; 64]),
            timestamp: DateTime::from_timestamp(1_700_000_000, 0).expect("a valid timestamp"),
            slot: 300_000_001,
            transaction_index: Some(2),
        },
        inner_instructions: payloads
            .into_iter()
            .map(|data| InnerInstructionPayload {
                program_id: program(),
                data,
            })
            .collect(),
    }
}

fn extract(payloads: Vec<Vec<u8>>) -> ExtractedEvents {
    extract_wire_events(&transaction(payloads), &program())
}

fn only_event(payload: Vec<u8>) -> DammV2WireEvent {
    let mut outcome = extract(vec![payload]);
    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    assert!(outcome.unknown.is_empty(), "{:?}", outcome.unknown);
    assert_eq!(outcome.events.len(), 1, "{:?}", outcome.events);
    outcome.events.remove(0).event
}

fn update_reward_duration() -> Vec<u8> {
    let mut body = vec![1u8; 32];
    body.push(1);
    body.extend_from_slice(&604_800u64.to_le_bytes());
    body.extend_from_slice(&1_209_600u64.to_le_bytes());
    event_cpi(discriminator_update_reward_duration(), &body)
}

fn update_reward_funder() -> Vec<u8> {
    let body = [&[1u8; 32][..], &[0], &[2u8; 32], &[3u8; 32]].concat();
    event_cpi(discriminator_update_reward_funder(), &body)
}

fn withdraw_dead_liquidity_reward() -> Vec<u8> {
    let body = [&[1u8; 32][..], &[2u8; 32], &42_000u64.to_le_bytes()].concat();
    event_cpi(discriminator_withdraw_dead_liquidity_reward(), &body)
}

fn set_pool_status() -> Vec<u8> {
    let body = [&[9u8; 32][..], &[1]].concat();
    event_cpi(discriminator_set_pool_status(), &body)
}

fn decoded_events() -> [([u8; DISCRIMINATOR_LEN], &'static str); 19] {
    [
        (discriminator_swap2(), "EvtSwap2"),
        (discriminator_liquidity_change(), "EvtLiquidityChange"),
        (discriminator_claim_position_fee(), "EvtClaimPositionFee"),
        (discriminator_claim_reward(), "EvtClaimReward"),
        (discriminator_claim_protocol_fee(), "EvtClaimProtocolFee"),
        (discriminator_initialize_reward(), "EvtInitializeReward"),
        (discriminator_fund_reward(), "EvtFundReward"),
        (
            discriminator_withdraw_ineligible_reward(),
            "EvtWithdrawIneligibleReward",
        ),
        (
            discriminator_update_reward_duration(),
            "EvtUpdateRewardDuration",
        ),
        (
            discriminator_update_reward_funder(),
            "EvtUpdateRewardFunder",
        ),
        (
            discriminator_withdraw_dead_liquidity_reward(),
            "EvtWithdrawDeadLiquidityReward",
        ),
        (discriminator_split_position3(), "EvtSplitPosition3"),
        (discriminator_create_position(), "EvtCreatePosition"),
        (discriminator_close_position(), "EvtClosePosition"),
        (discriminator_lock_position(), "EvtLockPosition"),
        (
            discriminator_permanent_lock_position(),
            "EvtPermanentLockPosition",
        ),
        (discriminator_initialize_pool(), "EvtInitializePool"),
        (discriminator_set_pool_status(), "EvtSetPoolStatus"),
        (discriminator_update_pool_fees(), "EvtUpdatePoolFees"),
    ]
}

/// Mutation: two discriminators exchanged in `dispatch`.
#[test]
fn the_four_events_without_a_capture_come_out_under_their_own_type() {
    match only_event(update_reward_duration()) {
        DammV2WireEvent::UpdateRewardDuration(e) => {
            assert_eq!(e.pool, pk(1));
            assert_eq!(e.reward_index, 1);
            assert_eq!(e.old_reward_duration, 604_800);
            assert_eq!(e.new_reward_duration, 1_209_600);
        }
        other => panic!("{other:?}"),
    }
    match only_event(update_reward_funder()) {
        DammV2WireEvent::UpdateRewardFunder(e) => {
            assert_eq!(e.pool, pk(1));
            assert_eq!(e.reward_index, 0);
            assert_eq!(e.old_funder, pk(2));
            assert_eq!(e.new_funder, pk(3));
        }
        other => panic!("{other:?}"),
    }
    match only_event(withdraw_dead_liquidity_reward()) {
        DammV2WireEvent::WithdrawDeadLiquidityReward(e) => {
            assert_eq!(e.pool, pk(1));
            assert_eq!(e.reward_mint, pk(2));
            assert_eq!(e.amount, 42_000);
        }
        other => panic!("{other:?}"),
    }
    match only_event(set_pool_status()) {
        DammV2WireEvent::SetPoolStatus(e) => {
            assert_eq!(e.pool, pk(9));
            assert_eq!(e.status, 1);
        }
        other => panic!("{other:?}"),
    }
}

/// Mutation: `translate_extracted_events` no longer pushing what it translated.
#[test]
fn the_four_events_reach_their_domain_event() {
    let domain = |payload| {
        let outcome = MeteoraDammV2::new()
            .extract_events(&transaction(vec![payload]))
            .expect("extract");
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
        assert_eq!(outcome.events.len(), 1, "{:?}", outcome.events);
        outcome.events.into_iter().next().expect("one event")
    };

    assert!(matches!(
        domain(update_reward_duration()),
        DomainEvent::MeteoraDammV2(MeteoraDammV2Event::UpdateRewardDuration(_))
    ));
    assert!(matches!(
        domain(update_reward_funder()),
        DomainEvent::MeteoraDammV2(MeteoraDammV2Event::UpdateRewardFunder(_))
    ));
    assert!(matches!(
        domain(withdraw_dead_liquidity_reward()),
        DomainEvent::MeteoraDammV2(MeteoraDammV2Event::WithdrawDeadLiquidityReward(_))
    ));
    assert!(matches!(
        domain(set_pool_status()),
        DomainEvent::MeteoraDammV2(MeteoraDammV2Event::SetPoolStatus(_))
    ));
}

/// Mutation: two names exchanged in `dispatch`, or a `BorshFailed` no longer pushed.
#[test]
fn an_unreadable_payload_is_a_failure_under_its_event_name() {
    for (discriminator, name) in decoded_events() {
        let outcome = extract(vec![event_cpi(discriminator, &[])]);

        assert!(outcome.events.is_empty(), "{name}: {:?}", outcome.events);
        assert!(outcome.unknown.is_empty(), "{name}: {:?}", outcome.unknown);
        match outcome.failures.as_slice() {
            [
                ExtractFailure::Borsh {
                    event_name,
                    discriminator: reported,
                    ..
                },
            ] => {
                assert_eq!(*event_name, name);
                assert_eq!(*reported, discriminator, "{name}");
            }
            other => panic!("{name}: {other:?}"),
        }
    }
}

/// Mutation: the name dropped by `map_extractor_failure`.
#[test]
fn an_unreadable_payload_keeps_its_name_out_of_the_extractor() {
    let tx = transaction(vec![event_cpi(discriminator_set_pool_status(), &[])]);

    let outcome = MeteoraDammV2::new().extract_events(&tx).expect("extract");

    assert!(outcome.events.is_empty(), "{:?}", outcome.events);
    assert!(
        matches!(
            outcome.failures.as_slice(),
            [ExtractionFailure::Borsh {
                event_name: "EvtSetPoolStatus",
                ..
            }]
        ),
        "{:?}",
        outcome.failures
    );
}

/// A payload that grew on-chain must not read as its old shape.
/// Mutation: `deserialize` stopping at the last field instead of `try_from_slice`.
#[test]
fn a_payload_too_long_for_its_event_is_a_failure() {
    let mut payload = set_pool_status();
    payload.push(0);

    let outcome = extract(vec![payload]);

    assert!(outcome.events.is_empty(), "{:?}", outcome.events);
    assert!(
        matches!(
            outcome.failures.as_slice(),
            [ExtractFailure::Borsh {
                event_name: "EvtSetPoolStatus",
                ..
            }]
        ),
        "{:?}",
        outcome.failures
    );
}

/// Mutation: `event_index` numbering only the payloads that decoded.
#[test]
fn a_payload_that_fails_or_is_unknown_keeps_its_index() {
    let outcome = extract(vec![
        event_cpi(discriminator_set_pool_status(), &[]),
        event_cpi([9u8; DISCRIMINATOR_LEN], &[]),
        set_pool_status(),
    ]);

    assert!(
        matches!(
            outcome.failures.as_slice(),
            [ExtractFailure::Borsh {
                event_name: "EvtSetPoolStatus",
                ..
            }]
        ),
        "{:?}",
        outcome.failures
    );
    assert!(
        matches!(
            outcome.unknown.as_slice(),
            [UnknownEvent { discriminator }] if *discriminator == [9u8; DISCRIMINATOR_LEN]
        ),
        "{:?}",
        outcome.unknown
    );
    let indices: Vec<u16> = outcome.events.iter().map(|e| e.event_index).collect();
    assert_eq!(indices, [2]);
}

/// Mutation: `Dispatch::Unknown` counted as a failure.
#[test]
fn an_unknown_discriminator_is_unknown_and_no_failure() {
    let outcome = extract(vec![event_cpi([9u8; DISCRIMINATOR_LEN], &[1, 2, 3])]);

    assert!(outcome.events.is_empty(), "{:?}", outcome.events);
    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    assert!(
        matches!(
            outcome.unknown.as_slice(),
            [UnknownEvent { discriminator }] if *discriminator == [9u8; DISCRIMINATOR_LEN]
        ),
        "{:?}",
        outcome.unknown
    );
}

/// Mutation: `translate_extracted_events` no longer carrying the unknown events over.
#[test]
fn an_unknown_discriminator_stays_unknown_out_of_the_extractor() {
    let tx = transaction(vec![event_cpi([9u8; DISCRIMINATOR_LEN], &[])]);

    let outcome = MeteoraDammV2::new().extract_events(&tx).expect("extract");

    assert!(outcome.events.is_empty(), "{:?}", outcome.events);
    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    assert_eq!(outcome.unknown.len(), 1, "{:?}", outcome.unknown);
    assert_eq!(outcome.unknown[0].protocol, Protocol::MeteoraDammV2);
    assert_eq!(outcome.unknown[0].discriminator, [9u8; DISCRIMINATOR_LEN]);
}

/// Mutation: the deprecated split decoded instead of dropped unread.
#[test]
fn an_unreadable_deprecated_split_is_dropped_and_no_failure() {
    let outcome = extract(vec![event_cpi(discriminator_split_position2(), &[])]);

    assert!(outcome.events.is_empty(), "{:?}", outcome.events);
    assert!(outcome.unknown.is_empty(), "{:?}", outcome.unknown);
    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
}

/// Mutation: the index cast with `as u16`, which stores the last event as number 0.
#[test]
fn the_payload_past_the_last_index_is_a_failure_and_not_a_truncated_index() {
    let ordinary = vec![0u8; DISCRIMINATOR_LEN];
    let mut payloads = vec![ordinary; usize::from(u16::MAX)];
    payloads.push(set_pool_status());
    payloads.push(set_pool_status());
    let tx = transaction(payloads);

    let outcome = extract_wire_events(&tx, &program());

    let indices: Vec<u16> = outcome.events.iter().map(|e| e.event_index).collect();
    assert_eq!(indices, [u16::MAX]);
    assert!(
        matches!(
            outcome.failures.as_slice(),
            [ExtractFailure::EventIndexOverflow { index: 65_536 }]
        ),
        "{:?}",
        outcome.failures
    );

    let domain = MeteoraDammV2::new().extract_events(&tx).expect("extract");
    assert!(
        matches!(
            domain.failures.as_slice(),
            [ExtractionFailure::EventIndexOverflow { index: 65_536 }]
        ),
        "{:?}",
        domain.failures
    );
    assert!(
        matches!(
            domain.events.as_slice(),
            [DomainEvent::MeteoraDammV2(MeteoraDammV2Event::SetPoolStatus(e))]
                if e.event_index == u16::MAX
        ),
        "{:?}",
        domain.events
    );
}
