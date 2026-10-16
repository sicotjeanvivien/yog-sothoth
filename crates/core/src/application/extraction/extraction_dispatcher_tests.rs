use super::*;

/// ⚠️ A spending decision: watching the DLMM stub pays a firehose for nothing.
/// Mutation: `MeteoraDlmm::is_implemented` returning `true`.
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

/// Mutation: `implemented_protocols` returning `Protocol::all()`.
#[test]
fn the_domain_names_more_protocols_than_extraction_handles() {
    let dispatcher = ExtractionDispatcher::new();

    assert!(
        dispatcher.implemented_protocols().len() < Protocol::all().len(),
        "if this fails because DLMM landed, delete this test in the change that flips its `is_implemented`"
    );
}
