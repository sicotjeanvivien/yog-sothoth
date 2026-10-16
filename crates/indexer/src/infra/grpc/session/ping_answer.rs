//! The answer to a server ping.

use tokio::sync::mpsc;
use yellowstone_grpc_proto::prelude::{SubscribeRequest, SubscribeRequestPing};

use crate::infra::grpc::metrics::{GrpcListenerMetrics, PingReplyFailure};

use super::log;

/// The `id` of a ping answer, echoed in the server's pong. Nothing correlates
/// on it; `1` is what the reference client example and Alchemy send.
pub(super) const PING_REPLY_ID: i32 = 1;

/// Answers server pings on the outbound half of the stream.
pub(super) struct PingAnswer {
    /// Held for the session's life even when idle: dropping it half-closes the
    /// request stream, which a server may read as the end of the exchange.
    outbound: mpsc::Sender<SubscribeRequest>,
}

impl PingAnswer {
    pub(super) fn new(outbound: mpsc::Sender<SubscribeRequest>) -> Self {
        Self { outbound }
    }

    /// Answer a server ping with a request carrying **only** `ping`; a provider may close a client that does not answer.
    /// ⚠️ A `from_slot` or a filter riding along would re-issue or replace the subscription.
    /// ⚠️ `try_send`, never `send().await`: waiting in `select!`'s body would deafen it to shutdown.
    pub(super) fn answer(&self) {
        let answer = SubscribeRequest {
            ping: Some(SubscribeRequestPing { id: PING_REPLY_ID }),
            ..Default::default()
        };

        if let Err(error) = self.outbound.try_send(answer) {
            let failure = match error {
                mpsc::error::TrySendError::Full(_) => PingReplyFailure::OutboundFull,
                mpsc::error::TrySendError::Closed(_) => PingReplyFailure::OutboundClosed,
            };
            GrpcListenerMetrics::record_ping_reply_unsent(failure);
            log::ping_answer_unsent(failure.as_str());
        }
    }
}
