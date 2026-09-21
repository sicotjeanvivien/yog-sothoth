//! What the configuration entails, said out loud before the daemon works.
//!
//! **Not "the start-up logs".** A name for the mechanism would take anything
//! that gets printed, and this file has a narrower subject: the things an
//! operator *cannot read off the variables they set*, because they follow from
//! a combination of them.
//!
//! Three of those exist today, and each is a pair — **state it, then object if
//! the combination deserves an objection**:
//!
//! - which of the four `(source, scope)` couples is running. Two variables,
//!   four meanings, and nothing else in the crate names the one in force;
//! - that one of those couples boots and cannot keep up. It is not an error,
//!   so it cannot be a refusal; it is not visible either, so it cannot be
//!   silence;
//! - what the health probe reads beside what ingestion touches — and whether
//!   they are the same address, which makes the dashboard's two halves fail
//!   together while the configuration still looks like two endpoints.
//!
//! **Why not in `init.rs`.** That file wires dependencies. Building a log
//! string and comparing two endpoints are neither of those, and they were
//! there only because the values were already in scope. The test they hide is
//! the giveaway: `probe_shares_ingestion_address` needed a live `Database` to
//! reach while it sat inside the reporter's constructor, so nothing covered
//! it. It is now a method on [`Config`] and this file only says what it found.

use tracing::{info, warn};

use crate::bootstrap::{Config, IngestScope, TransactionArrival};

/// Name the acquisition couple that is running.
///
/// ⚠️ **The first line the process writes, because it is the first question a
/// reader has.** Two acquisition models exist and one is running; from here on
/// nothing else in the crate names which. The two `as_str` were written for
/// the refusals of a validator that no longer exists — their remaining reader
/// is this line, and it is a better one: a refusal is read once, a running
/// mode every time something looks wrong.
pub(super) fn log_ingestion_mode(config: &Config) {
    info!(
        source = config.transaction_arrival.source().as_str(),
        scope = config.scope.as_str(),
        "ingestion mode"
    );
}

/// Object to the one couple that boots and cannot keep up.
///
/// `logsSubscribe` on a program id delivers everything that program does, and
/// the RPC path then fetches each transaction back — measured at ~200 in 30 s
/// against a ~10 req/s tier. Nothing stops: fetch failures are skip-and-logged
/// per transaction, so the process stays up and the metrics stay plausible
/// while most of what it sees is dropped.
///
/// A `check_supported` used to refuse that couple, for a different reason — an
/// empty target set — and that reason is genuinely fixed. What went with the
/// refusal was the only loud signal an operator got, and this warning puts it
/// back at the cost of one branch.
pub(super) fn warn_saturating_couple(config: &Config) {
    if matches!(
        (&config.transaction_arrival, config.scope),
        (TransactionArrival::Fetched { .. }, IngestScope::Protocols)
    ) {
        warn!(
            "INGEST_SOURCE=rpc with INGEST_SCOPE=protocols subscribes to the whole program and fetches every transaction back, one request each. On a rate-limited endpoint most will be dropped and counted as fetch failures, with the process still up. INGEST_SOURCE=grpc is the mode this scope is for."
        );
    }
}

/// Print the probe's endpoint beside **everything ingestion touches**.
///
/// Everything, and not just the stream: on the notify-then-ask path ingestion
/// also holds `INGEST_TRANSACTION`, the very endpoint the probe used to share
/// until 21 September 2026, and a line omitting it would let an operator read
/// an independence that endpoint denies.
///
/// The message states what the numbers are, and nothing about whether they are
/// independent — that claim belongs to [`warn_probe_not_independent`], which
/// checks it. It used to be asserted here whatever the addresses were, which
/// made it false on a fresh clone, where `.env.example` points both at the same
/// public host. A message is also what survives in an aggregator, where the
/// fields beside it do not.
pub(super) fn log_probe_endpoints(config: &Config) {
    let ingestion = match config.transaction_arrival.fetched_from() {
        Some(fetch) => format!(
            "{} (stream) + {fetch} (getTransaction)",
            config.ingest_stream
        ),
        None => format!("{} (stream)", config.ingest_stream),
    };
    info!(
        probe = %config.network_status,
        %ingestion,
        "network status probe initialized — the chain reference the panel's slot and latency come from"
    );
}

/// Object when the probe reads an address ingestion already uses.
///
/// The panel it feeds combines two questions that are worth combining only
/// because they fail apart — *is the chain advancing* and *are we keeping up*.
/// One provider behind both turns them into one question again, silently,
/// since the configuration still shows two endpoints.
///
/// What is compared, and what is not, is [`Config::probe_shares_ingestion_address`]'s
/// business; the message repeats the limit rather than implying a guarantee.
pub(super) fn warn_probe_not_independent(config: &Config) {
    if config.probe_shares_ingestion_address() {
        warn!(
            "NETWORK_STATUS_URL is the address ingestion already uses, so the dashboard's two \
             halves share one provider: the day it drops, the chain reading and the freshness \
             verdict go red together and neither says which failed. Point it elsewhere — the \
             probe costs one request every fifteen seconds. Compared as written; two spellings \
             of the same host would not be caught here."
        );
    }
}
