use crate::application::LiveScope;
use crate::http::dto::request::CreateLiveTokenBody;
use crate::http::error::ApiError;

fn parse(body: &str) -> Result<LiveScope, ApiError> {
    CreateLiveTokenBody::parse(body.as_bytes())
}

fn refusal(body: &str) -> String {
    match parse(body) {
        Err(ApiError::BadRequest(message)) => message,
        other => panic!("expected a 400, got {other:?}"),
    }
}

/// Mutation: the path kept as written rather than rebuilt from the protocol it names.
#[test]
fn a_body_names_the_scope_of_its_stream() {
    assert_eq!(
        parse(r#"{"path":"/api/live/protocols/meteora_damm_v2","topics":["signals","pool-creations"]}"#)
            .unwrap(),
        LiveScope {
            path: "/api/live/protocols/meteora_damm_v2".to_string(),
            topics: vec!["signals".to_string(), "pool-creations".to_string()],
        }
    );
    assert_eq!(
        parse(r#"{"path":"/api/live","topics":["signals"]}"#)
            .unwrap()
            .path,
        "/api/live"
    );
}

// One refusal per rule, each named by its reason.

#[test]
fn a_path_outside_the_live_stream_is_refused() {
    assert_eq!(
        refusal(r#"{"path":"/api/signals","topics":["signals"]}"#),
        "`path` is no path of the live stream: /api/signals"
    );
}

#[test]
fn an_unknown_protocol_is_refused() {
    assert_eq!(
        refusal(r#"{"path":"/api/live/protocols/uniswap","topics":["signals"]}"#),
        "unknown protocol: uniswap"
    );
}

#[test]
fn a_topic_the_stream_does_not_serve_is_refused() {
    assert!(
        refusal(r#"{"path":"/api/live","topics":["swaps"]}"#)
            .starts_with("`topics`: `swaps` is not streamed")
    );
}

#[test]
fn no_topic_is_refused() {
    assert_eq!(
        refusal(r#"{"path":"/api/live","topics":[]}"#),
        "`topics` must not be empty"
    );
}

#[test]
fn an_unknown_field_is_refused() {
    assert!(
        refusal(r#"{"path":"/api/live","topics":["signals"],"ttl":600}"#)
            .starts_with("invalid body: unknown field `ttl`")
    );
}
