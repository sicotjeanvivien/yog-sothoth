pub mod events;
pub mod extractor;
pub(super) mod translator;

use solana_pubkey::Pubkey;

use crate::CoreResult;
use crate::application::extraction::outcome::{ExtractionFailure, UnknownEventInfo};
use crate::application::extraction::{EventExtractor, ExtractionOutcome, OnChainTransaction};
use crate::domain::{Protocol, TransactionPosition};

use self::extractor::extract_wire_events;
use self::translator::translate_wire_event;

/// Meteora DAMM v2 protocol handler (x·y=k + dynamic fees + NFT positions).
pub struct MeteoraDammV2 {
    protocol: Protocol,
    program_id: Pubkey,
}

impl MeteoraDammV2 {
    pub fn new() -> Self {
        let protocol = Protocol::MeteoraDammV2;
        Self {
            protocol,
            program_id: protocol.program_id(),
        }
    }
}

impl Default for MeteoraDammV2 {
    fn default() -> Self {
        Self::new()
    }
}

impl EventExtractor for MeteoraDammV2 {
    fn program_id(&self) -> Pubkey {
        self.program_id
    }

    fn extract_events(&self, tx: &OnChainTransaction) -> CoreResult<ExtractionOutcome> {
        // Step 1: extract wire events from the inner-instruction payloads.
        let wire_outcome = extract_wire_events(tx, &self.program_id);

        // Step 2: translate each wire event into a domain event. The
        // coordinate comes ready-made — whichever source filled it.
        translate_extracted_events(wire_outcome, self.protocol, tx.position)
    }
}

/// Glue between the wire-event extraction layer and the translation layer.
///
/// Produces one domain event per successfully translated wire event.
/// Translation failures are reported in `failures`; the loop continues on each
/// failure (skip-and-log).
fn translate_extracted_events(
    wire_outcome: extractor::ExtractedEvents,
    protocol: Protocol,
    transaction_position: TransactionPosition,
) -> CoreResult<ExtractionOutcome> {
    let mut outcome = ExtractionOutcome::default();

    // Carry over decode-time failures and unknowns into the protocol-agnostic
    // ExtractionOutcome.
    for unknown in wire_outcome.unknown {
        outcome.unknown.push(UnknownEventInfo {
            protocol,
            discriminator: unknown.discriminator,
        });
    }

    for failure in wire_outcome.failures {
        outcome.failures.push(map_extractor_failure(failure));
    }

    for indexed in wire_outcome.events.iter() {
        let event_position = transaction_position.at(indexed.event_index);

        match translate_wire_event(&indexed.event, event_position) {
            Ok(domain) => outcome.events.push(domain),
            Err(e) => {
                outcome.failures.push(ExtractionFailure::Translation {
                    event_name: wire_event_name(&indexed.event),
                    reason: e.to_string(),
                });
            }
        }
    }

    Ok(outcome)
}

fn wire_event_name(wire: &events::DammV2WireEvent) -> &'static str {
    match wire {
        events::DammV2WireEvent::Swap2(_) => "EvtSwap2",
        events::DammV2WireEvent::LiquidityChange(_) => "EvtLiquidityChange",
        events::DammV2WireEvent::ClaimPositionFee(_) => "EvtClaimPositionFee",
        events::DammV2WireEvent::ClaimReward(_) => "EvtClaimReward",
        events::DammV2WireEvent::ClaimProtocolFee(_) => "EvtClaimProtocolFee",
        events::DammV2WireEvent::InitializeReward(_) => "EvtInitializeReward",
        events::DammV2WireEvent::FundReward(_) => "EvtFundReward",
        events::DammV2WireEvent::WithdrawIneligibleReward(_) => "EvtWithdrawIneligibleReward",
        events::DammV2WireEvent::UpdateRewardDuration(_) => "EvtUpdateRewardDuration",
        events::DammV2WireEvent::UpdateRewardFunder(_) => "EvtUpdateRewardFunder",
        events::DammV2WireEvent::WithdrawDeadLiquidityReward(_) => "EvtWithdrawDeadLiquidityReward",
        events::DammV2WireEvent::SplitPosition3(_) => "EvtSplitPosition3",
        events::DammV2WireEvent::CreatePosition(_) => "EvtCreatePosition",
        events::DammV2WireEvent::ClosePosition(_) => "EvtClosePosition",
        events::DammV2WireEvent::LockPosition(_) => "EvtLockPosition",
        events::DammV2WireEvent::PermanentLockPosition(_) => "EvtPermanentLockPosition",
        events::DammV2WireEvent::InitializePool(_) => "EvtInitializePool",
        events::DammV2WireEvent::SetPoolStatus(_) => "EvtSetPoolStatus",
        events::DammV2WireEvent::UpdatePoolFees(_) => "EvtUpdatePoolFees",
    }
}

fn map_extractor_failure(failure: extractor::ExtractFailure) -> ExtractionFailure {
    match failure {
        extractor::ExtractFailure::AnchorDecode { source } => {
            ExtractionFailure::AnchorDecode(source.to_string())
        }
        extractor::ExtractFailure::Borsh {
            event_name, reason, ..
        } => ExtractionFailure::Borsh { event_name, reason },
        extractor::ExtractFailure::EventIndexOverflow { index } => {
            ExtractionFailure::EventIndexOverflow { index }
        }
    }
}
