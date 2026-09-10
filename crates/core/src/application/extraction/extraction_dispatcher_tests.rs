use super::*;

/// ⚠️ This test carries a spending decision, not a naming one.
///
/// Watching a protocol means a program-wide subscription — an RPC quota spent
/// per transaction, or bandwidth billed by the byte. The DLMM extractor is a
/// stub returning an empty outcome, and `DomainEvent` has no variant for it, so
/// every one of those transactions would be decoded and discarded.
///
/// Put in failure before being trusted: making `MeteoraDlmm::is_implemented`
/// return `true` turns this red. Without that, the day someone deletes the
/// override because "DLMM is nearly done", nothing says the ingestion started
/// paying for a firehose.
#[test]
fn a_stubbed_protocol_is_not_something_to_subscribe_to() {
    let dispatcher = ExtractionDispatcher::new();

    assert_eq!(
        dispatcher.implemented_protocols(),
        vec![Protocol::MeteoraDammV2],
        "only protocols whose extraction is written may be subscribed to — \
         MeteoraDlmm's `extract_events` returns an empty outcome"
    );
}

/// The other half, and it is not the same statement: the domain names more
/// protocols than the ingestion watches, and that gap is the point. A build
/// where the two lists coincide has either implemented every protocol or lost
/// the distinction.
#[test]
fn the_domain_names_more_protocols_than_extraction_handles() {
    let dispatcher = ExtractionDispatcher::new();

    assert!(
        dispatcher.implemented_protocols().len() < Protocol::all().len(),
        "if this fails because DLMM landed, delete it with the stub's override"
    );
}
