pub mod events;
pub mod extractor;
pub(super) mod translator;

use chrono::{DateTime, Utc};
use solana_signature::Signature;

use crate::CoreResult;
use crate::application::extraction::meteora::{extract_signature, extract_timestamp};
use crate::application::extraction::outcome::{ExtractionFailure, UnknownEventInfo};
use crate::application::extraction::{EventExtractor, ExtractionOutcome};
use crate::domain::Protocol;
use crate::solana_types::{EncodedConfirmedTransactionWithStatusMeta, UiInstruction};

use self::extractor::extract_wire_events;
use self::translator::{collect_pre_event_instruction_slices, translate_wire_event};

/// Meteora DAMM v2 protocol handler (x·y=k + dynamic fees + NFT positions).
pub struct MeteoraDammV2 {
    protocol: Protocol,
    program_id_str: String,
}

impl MeteoraDammV2 {
    pub fn new() -> Self {
        let protocol = Protocol::MeteoraDammV2;
        let program_id_str = protocol.program_id().to_string();
        Self {
            protocol,
            program_id_str,
        }
    }
}

impl Default for MeteoraDammV2 {
    fn default() -> Self {
        Self::new()
    }
}

impl EventExtractor for MeteoraDammV2 {
    fn program_id(&self) -> &str {
        &self.program_id_str
    }

    fn extract_events(
        &self,
        tx: &EncodedConfirmedTransactionWithStatusMeta,
    ) -> CoreResult<ExtractionOutcome> {
        let signature = extract_signature(tx)?;
        let timestamp = extract_timestamp(tx)?;

        // Step 1: extract wire events from inner instructions.
        let wire_outcome = extract_wire_events(tx, &self.program_id_str);

        // Step 2: precompute, in the same order, the slice of instructions
        // preceding each self-CPI (used for mint extraction by the translator).
        let transfer_groups = collect_pre_event_instruction_slices(tx, &self.program_id_str);

        // Step 3: translate each wire event into a domain event.
        translate_extracted_events(
            wire_outcome,
            transfer_groups,
            self.protocol,
            signature,
            timestamp,
        )
    }
}

/// Glue between the wire-event extraction layer and the translation layer.
///
/// Walks `wire_events` and `transfer_groups` together, producing one domain
/// event per successfully translated wire event. Translation failures are
/// reported in `failures`; the loop continues on each failure (skip-and-log).
fn translate_extracted_events(
    wire_outcome: extractor::ExtractedEvents,
    transfer_groups: Vec<Vec<&UiInstruction>>,
    protocol: Protocol,
    signature: Signature,
    timestamp: DateTime<Utc>,
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

    // Sanity: the transfer_groups list should have at least as many entries
    // as recognized wire events. If not, we have an alignment problem
    // (probably a transaction shape we don't expect) — record translation
    // failures for the unalignable suffix.
    let event_count = wire_outcome.events.len();
    let group_count = transfer_groups.len();

    for (idx, wire) in wire_outcome.events.iter().enumerate() {
        let transfer_group = transfer_groups
            .get(idx)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        match translate_wire_event(wire, transfer_group, signature, timestamp) {
            Ok(domain) => outcome.events.push(domain),
            Err(e) => {
                outcome.failures.push(ExtractionFailure::Translation {
                    event_name: wire_event_name(wire),
                    reason: e.to_string(),
                });
            }
        }
    }

    if event_count > group_count {
        // Diagnostic only — already covered as per-event failures above.
        // We keep the check explicit to make the case visible if it ever happens.
    }

    Ok(outcome)
}

fn wire_event_name(wire: &events::DammV2WireEvent) -> &'static str {
    match wire {
        events::DammV2WireEvent::Swap2(_) => "EvtSwap2",
        events::DammV2WireEvent::LiquidityChange(_) => "EvtLiquidityChange",
        events::DammV2WireEvent::ClaimPositionFee(_) => "EvtClaimPositionFee",
        events::DammV2WireEvent::ClaimReward(_) => "EvtClaimReward",
        events::DammV2WireEvent::CreatePosition(_) => "EvtCreatePosition",
        events::DammV2WireEvent::ClosePosition(_) => "EvtClosePosition",
        events::DammV2WireEvent::LockPosition(_) => "EvtLockPosition",
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
    }
}
