use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use chrono::{TimeZone, Utc};
use solana_signature::Signature;
use tokio::sync::{Semaphore, broadcast};
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;
use yog_core::domain::{
    LiquidityChangeEvent, LiquidityChangeTable, LiveRecord, MeteoraDammV2ClosePositionEvent,
    MeteoraDammV2CreatePositionEvent, MeteoraDammV2LiquidityEvent, MeteoraDammV2LiquidityEventKind,
    MeteoraDammV2PositionChange, PositionChangeEvent, PositionChangeTable, Protocol,
};

use super::{
    LiquidityChangeEventResponse, PositionChangeEventResponse, SignalResponse, TopicStream,
    event_stream, events_topic, live_sse, open_live, signals_topic,
};
use crate::application::{EnrichedSignal, EnrichedToken, LiveEvent, Paired};
use crate::http::{build_router, cursor::LiveCursor, dto::request::LiveTopic};
use crate::testing::{app_state, get, make_metadata, make_signal_record, pk};

/// Mutation: a topic the stream does not serve accepted — the connection opens and stays silent.
#[tokio::test]
async fn a_connection_it_cannot_serve_is_refused_in_problem_details() {
    for (uri, reason) in [
        ("/api/live", "missing field `topics`"),
        ("/api/live?topics=", "`topics` must not be empty"),
        (
            "/api/live?topics=swaps",
            "`swaps` is not streamed; served: signals, liquidity-changes, pool-creations, \
             position-changes, position-fee-claims, protocol-fee-claims, reward-claims, \
             reward-changes, pool-changes",
        ),
        ("/api/live?topics=signals,swaps", "`swaps` is not streamed"),
        ("/api/live?topics=signals,signals", "names `signals` twice"),
        ("/api/live?topics=signals,", "`` is not streamed"),
        ("/api/live?topics=signals&pool=x", "unknown field `pool`"),
        (
            "/api/live/protocols/raydium?topics=signals",
            "unknown protocol: raydium",
        ),
    ] {
        let (status, content_type, detail) = get(uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {detail}");
        assert_eq!(content_type, "application/problem+json", "{uri}");
        assert!(detail.contains(reason), "{uri}: {detail}");
    }
}

/// The control of the refusals above, with a `Last-Event-ID` a resume would send: it is not
/// read, so even one that decodes to nothing opens the stream.
#[tokio::test]
async fn a_served_connection_opens_whatever_its_last_event_id() {
    const EVERY_TOPIC: &str = "signals,liquidity-changes,pool-creations,position-changes,\
                               position-fee-claims,protocol-fee-claims,reward-claims,\
                               reward-changes,pool-changes";
    for uri in [
        "/api/live?topics=signals".to_string(),
        "/api/live/protocols/meteora_damm_v2?topics=signals".to_string(),
        format!("/api/live?topics={EVERY_TOPIC}"),
        format!("/api/live/protocols/meteora_damm_v2?topics={EVERY_TOPIC}"),
    ] {
        let request = Request::builder()
            .uri(&uri)
            .header("last-event-id", "not-a-cursor")
            .body(Body::empty())
            .unwrap();
        let response = build_router(app_state(), vec![])
            .oneshot(request)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{uri}");
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/event-stream",
            "{uri}"
        );
    }
}

/// The whole body of a connection on `topics`, read until it ends, and whether its slot came back.
async fn read_live(topics: Vec<TopicStream>) -> (String, bool) {
    let slots = Arc::new(Semaphore::new(1));
    let slot = slots.clone().try_acquire_owned().expect("a free slot");
    let response = axum::response::IntoResponse::into_response(live_sse(
        topics,
        slot,
        CancellationToken::new(),
    ));
    assert_eq!(
        slots.available_permits(),
        0,
        "the connection holds its slot"
    );

    let body = tokio::time::timeout(
        Duration::from_secs(5),
        axum::body::to_bytes(response.into_body(), usize::MAX),
    )
    .await
    .expect("the connection ends")
    .expect("the body reads");
    (
        String::from_utf8(body.to_vec()).expect("text"),
        slots.available_permits() == 1,
    )
}

fn enriched(id: i64, seed: u8, protocol: Protocol) -> EnrichedSignal {
    let token = |mint, symbol| EnrichedToken {
        mint: Some(mint),
        metadata: Some(make_metadata(mint, symbol)),
    };
    let mut record = make_signal_record(id, pk(seed));
    record.signal.protocol = protocol;
    EnrichedSignal {
        record,
        token_a: token(pk(seed + 10), "AAA"),
        token_b: token(pk(seed + 20), "BBB"),
    }
}

/// The signals broadcast, then the source closed: what a connection would have received.
fn broadcast_then_close(signals: &[EnrichedSignal]) -> broadcast::Receiver<Arc<EnrichedSignal>> {
    let (feed, receiver) = broadcast::channel(8);
    for signal in signals {
        feed.send(Arc::new(signal.clone())).expect("a subscriber");
    }
    receiver
}

fn events(body: &str) -> Vec<&str> {
    body.split("\n\n")
        .filter(|event| !event.is_empty())
        .collect()
}

fn field<'a>(event: &'a str, name: &str) -> &'a str {
    event
        .lines()
        .find_map(|line| line.strip_prefix(name)?.strip_prefix(": "))
        .unwrap_or_else(|| panic!("no `{name}` in {event:?}"))
}

/// Mutation: the `id` reduced to the signal's own, the event left unnamed, `data` not the
/// signal's response, or the signal sent bare.
#[tokio::test]
async fn each_signal_comes_named_by_its_topic_with_the_connections_cursor() {
    let signals = [
        enriched(41, 1, Protocol::MeteoraDammV2),
        enriched(42, 2, Protocol::MeteoraDammV2),
    ];
    let receiver = broadcast_then_close(&signals);

    let (body, slot_back) = read_live(vec![signals_topic(receiver, None)]).await;

    assert!(slot_back);
    let events = events(&body);
    assert_eq!(events.len(), 2, "{body}");
    for (event, signal) in events.iter().zip(&signals) {
        assert_eq!(field(event, "event"), "signals");
        let cursor = LiveCursor::decode(field(event, "id")).expect("an opaque cursor");
        assert_eq!(cursor.position("signals"), Some(signal.record.id));
        let data: serde_json::Value = serde_json::from_str(field(event, "data")).expect("JSON");
        let expected = serde_json::to_value(SignalResponse::from(signal.clone())).expect("JSON");
        assert_eq!(data, expected);
        assert_eq!(data["tokenA"]["symbol"], "AAA");
        assert_eq!(data["tokenB"]["symbol"], "BBB");
    }
}

/// Mutation: the protocol filter dropped — DLMM's signal reaches a DAMM v2 connection.
#[tokio::test]
async fn a_protocols_connection_carries_its_signals_only() {
    let signals = [
        enriched(41, 1, Protocol::MeteoraDlmm),
        enriched(42, 2, Protocol::MeteoraDammV2),
    ];

    let (body, _) = read_live(vec![signals_topic(
        broadcast_then_close(&signals),
        Some(Protocol::MeteoraDammV2),
    )])
    .await;
    let events = events(&body);
    assert_eq!(events.len(), 1, "{body}");
    let data: serde_json::Value = serde_json::from_str(field(events[0], "data")).expect("JSON");
    assert_eq!(data["id"], 42);

    // The control: without a protocol, both come.
    let (body, _) = read_live(vec![signals_topic(broadcast_then_close(&signals), None)]).await;
    assert_eq!(self::events(&body).len(), 2, "{body}");
}

/// Mutation: opening without taking a slot — of two slots, the third connection opens. Closing
/// one lets the next client in. And the topics built before the slot is taken — the refused
/// client subscribes too.
#[tokio::test]
async fn connections_past_the_cap_are_refused_until_one_closes() {
    let slots = Arc::new(Semaphore::new(2));
    let (feed, _) = broadcast::channel::<Arc<EnrichedSignal>>(8);
    let shutdown = CancellationToken::new();
    let subscriptions = std::cell::Cell::new(0);
    let open = || {
        open_live(
            slots.clone(),
            || {
                subscriptions.set(subscriptions.get() + 1);
                vec![signals_topic(feed.subscribe(), None)]
            },
            shutdown.clone(),
        )
    };

    let first = open();
    let second = open();
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(second.status(), StatusCode::OK);

    let refused = open();
    assert_eq!(refused.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(refused.headers().contains_key(header::RETRY_AFTER));
    assert_eq!(
        refused.headers()[header::CONTENT_TYPE],
        "application/problem+json"
    );
    assert_eq!(
        subscriptions.get(),
        2,
        "the refused client never subscribes"
    );

    // The client leaves: its response, and the connection holding the slot, drop.
    drop(first);
    assert_eq!(open().status(), StatusCode::OK);
}

/// Mutation: a client past the topic's buffer kept on the connection. The source stays open here.
#[tokio::test]
async fn a_client_too_late_ends_its_connection_and_gives_its_slot_back() {
    let (feed, receiver) = broadcast::channel(1);
    for id in 1..=3 {
        feed.send(Arc::new(EnrichedSignal::bare(make_signal_record(
            id,
            pk(1),
        ))))
        .expect("a subscriber");
    }

    let (body, slot_back) = read_live(vec![signals_topic(receiver, None)]).await;

    assert_eq!(body, "");
    assert!(slot_back);
}

/// Mutation: a closed source no longer ending the connection, or the slot never given back.
#[tokio::test]
async fn a_closed_source_ends_the_connection_and_gives_its_slot_back() {
    let (feed, receiver) = broadcast::channel::<Arc<EnrichedSignal>>(8);
    drop(feed);

    let (body, slot_back) = read_live(vec![signals_topic(receiver, None)]).await;

    assert_eq!(body, "");
    assert!(slot_back);
}

fn liquidity_change(seed: u8) -> LiquidityChangeEvent {
    LiquidityChangeEvent::MeteoraDammV2(MeteoraDammV2LiquidityEvent {
        pool_address: pk(seed),
        signature: Signature::from([seed; 64]),
        timestamp: Utc.with_ymd_and_hms(2026, 11, 4, 12, 0, 0).unwrap(),
        slot: 9,
        transaction_index: None,
        event_index: 0,
        liquidity_event_kind: MeteoraDammV2LiquidityEventKind::Add,
        amount_a: 1,
        amount_b: 2,
        liquidity_delta: 3,
        reserve_a_after: 4,
        reserve_b_after: 5,
        position: pk(seed + 1),
        owner: pk(seed + 2),
    })
}

/// The events broadcast by their tables, then the source closed.
fn events_then_close<T: Copy, E>(
    events: Vec<(T, i64, E)>,
) -> broadcast::Receiver<Arc<LiveEvent<T, Paired<E>>>> {
    let (feed, receiver) = broadcast::channel(8);
    for (table, id, event) in events {
        let item = LiveEvent {
            table,
            record: LiveRecord {
                id,
                event: Paired::unpaired(event),
            },
        };
        assert!(feed.send(Arc::new(item)).is_ok(), "a subscriber");
    }
    receiver
}

fn liquidity_topic(
    events: Vec<(LiquidityChangeTable, i64, LiquidityChangeEvent)>,
    protocol: Option<Protocol>,
) -> TopicStream {
    event_stream::<_, _, LiquidityChangeEventResponse>(
        LiveTopic::LiquidityChanges,
        events_then_close(events),
        protocol,
    )
}

/// Mutation: the `id` reduced to the item's own — the last message of `signals,liquidity-changes`
/// forgets the other topic's position. And the event left unnamed or not its collection's item.
#[tokio::test]
async fn an_event_comes_as_its_collections_item_and_the_cursor_keeps_every_topic() {
    let event = liquidity_change(1);
    let signals = vec![signals_topic(
        broadcast_then_close(&[enriched(41, 1, Protocol::MeteoraDammV2)]),
        None,
    )];
    let liquidity = liquidity_topic(
        vec![(LiquidityChangeTable::MeteoraDammV2, 7, event.clone())],
        None,
    );

    let (body, slot_back) = read_live(signals.into_iter().chain([liquidity]).collect()).await;

    assert!(slot_back);
    let events = events(&body);
    assert_eq!(events.len(), 2, "{body}");
    let change = events
        .iter()
        .find(|e| field(e, "event") == "liquidity-changes")
        .expect("the liquidity change, named by its topic");
    let data: serde_json::Value = serde_json::from_str(field(change, "data")).expect("JSON");
    let expected =
        serde_json::to_value(LiquidityChangeEventResponse::from(Paired::unpaired(event)))
            .expect("JSON");
    assert_eq!(data, expected);
    let last = LiveCursor::decode(field(events[1], "id")).expect("an opaque cursor");
    assert_eq!(last.position("signals"), Some(41));
    assert_eq!(last.position("meteora_damm_v2_liquidity_events"), Some(7));
}

/// Mutation: the cursor kept per topic — two tables of `position-changes`, each at its own id 5,
/// collapse into one position.
#[tokio::test]
async fn the_cursor_keeps_one_position_per_table() {
    let at = Utc.with_ymd_and_hms(2026, 11, 4, 12, 0, 0).unwrap();
    let open = MeteoraDammV2CreatePositionEvent {
        pool_address: pk(1),
        signature: Signature::from([1; 64]),
        timestamp: at,
        slot: 9,
        transaction_index: None,
        event_index: 0,
        owner: pk(2),
        position: pk(3),
        position_nft_mint: pk(4),
    };
    let close = MeteoraDammV2ClosePositionEvent {
        pool_address: pk(1),
        signature: Signature::from([2; 64]),
        timestamp: at,
        slot: 9,
        transaction_index: None,
        event_index: 0,
        owner: pk(2),
        position: pk(3),
        position_nft_mint: pk(4),
    };
    let topic = event_stream::<_, _, PositionChangeEventResponse>(
        LiveTopic::PositionChanges,
        events_then_close(vec![
            (
                PositionChangeTable::MeteoraDammV2Open,
                5,
                PositionChangeEvent::MeteoraDammV2(MeteoraDammV2PositionChange::Open(open)),
            ),
            (
                PositionChangeTable::MeteoraDammV2Close,
                6,
                PositionChangeEvent::MeteoraDammV2(MeteoraDammV2PositionChange::Close(close)),
            ),
        ]),
        None,
    );

    let (body, _) = read_live(vec![topic]).await;

    let events = events(&body);
    assert_eq!(events.len(), 2, "{body}");
    assert!(
        events
            .iter()
            .all(|e| field(e, "event") == "position-changes")
    );
    let last = LiveCursor::decode(field(events[1], "id")).expect("an opaque cursor");
    assert_eq!(
        last.position("meteora_damm_v2_create_position_events"),
        Some(5)
    );
    assert_eq!(
        last.position("meteora_damm_v2_close_position_events"),
        Some(6)
    );
}

/// Mutation: the protocol filter dropped from the events — DAMM v2's change reaches a DLMM
/// connection.
#[tokio::test]
async fn a_protocols_connection_carries_its_events_only() {
    let events = || vec![(LiquidityChangeTable::MeteoraDammV2, 7, liquidity_change(1))];

    let (body, _) = read_live(vec![liquidity_topic(events(), Some(Protocol::MeteoraDlmm))]).await;
    assert_eq!(body, "");

    // The control: its own protocol's connection gets it.
    let (body, _) = read_live(vec![liquidity_topic(
        events(),
        Some(Protocol::MeteoraDammV2),
    )])
    .await;
    assert_eq!(self::events(&body).len(), 1, "{body}");
}

/// Mutation: a slot taken per topic — a connection on two topics fills a cap of two alone.
#[tokio::test]
async fn a_connection_takes_one_slot_whatever_its_topics() {
    let slots = Arc::new(Semaphore::new(2));
    let (signals, _) = broadcast::channel::<Arc<EnrichedSignal>>(8);
    let (changes, _) =
        broadcast::channel::<Arc<LiveEvent<LiquidityChangeTable, Paired<LiquidityChangeEvent>>>>(8);

    let response = open_live(
        slots.clone(),
        || {
            vec![
                signals_topic(signals.subscribe(), None),
                events_topic::<_, _, LiquidityChangeEventResponse>(
                    LiveTopic::LiquidityChanges,
                    &changes,
                    None,
                ),
            ]
        },
        CancellationToken::new(),
    );

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(slots.available_permits(), 1);
}

/// Mutation: every topic subscribed whatever the protocol — a DLMM connection, which no
/// `liquidity-changes` table is written for, keeps DAMM v2's poller reading.
#[tokio::test]
async fn a_protocol_without_a_table_in_the_collection_subscribes_to_nothing() {
    let (changes, _) =
        broadcast::channel::<Arc<LiveEvent<LiquidityChangeTable, Paired<LiquidityChangeEvent>>>>(8);
    let subscribe = |protocol| {
        events_topic::<_, _, LiquidityChangeEventResponse>(
            LiveTopic::LiquidityChanges,
            &changes,
            protocol,
        )
    };

    let dlmm = subscribe(Some(Protocol::MeteoraDlmm));
    assert_eq!(changes.receiver_count(), 0);

    let damm = subscribe(Some(Protocol::MeteoraDammV2));
    let every = subscribe(None);
    assert_eq!(changes.receiver_count(), 2);
    drop((dlmm, damm, every));
}
