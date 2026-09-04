# yog-indexer

Native binary. Long-lived process consuming Solana mainnet WebSocket events
and persisting indexed state — the ingest end of the platform.

For the workspace-level picture (dependency graph, conventions, database
roles, the add-a-protocol recipe), see [`crates/README.md`](../README.md).

---

## Layout

```
indexer/src/
├── application/
│   ├── services/          ← TransactionProcessor, EventPersistor + the
│   │                        per-protocol sub-persistors (meteora/damm_v2/),
│   │                        PoolMaintenance, WatchedPoolService, metrics
│   ├── reporter/          ← NetworkStatusReporter (Solana slot/latency snapshot)
│   └── workers/           ← IndexerWorker (bounded-concurrency consumer),
│                            subscription supervisor
├── infra/rpc/             ← RpcListener (WebSocket), SignatureDispatcher
│                            filter chain, TransactionFetcher (HTTP + FetchError)
├── bootstrap/             ← Config::load(), Daemon (lifecycle, task wiring,
│                            shutdown, init_event_persistor)
├── error/                 ← typed error per layer
├── utils/redact.rs        ← API-key scrubbing for logs
├── bin/inspect_logs.rs    ← ad-hoc debugging helper for raw log streams
└── main.rs
```

## The source adapters

`yog-core` extracts from an `OnChainTransaction` and never learns who filled it.
Filling it is this crate's job, one module per source, under `infra/rpc/`:

- `transaction_adapter.rs` turns a `getTransaction` response into that neutral
  shape. It sits beside `transaction_fetcher.rs` on purpose — the encoding and
  the adapter are **one contract** (the fetcher must ask for `JsonParsed`,
  because the adapter reads the `PartiallyDecoded` inner instructions only that
  encoding produces), and splitting them across crates is what would let the two
  drift;
- a Yellowstone gRPC source becomes a sibling module here, not a second path
  through extraction.

**What an adapter owes**, and how it is held to it: the order of the payloads it
produces becomes the persisted `event_index`, part of the unique key of every
event table. An adapter that reorders does not fail — it renumbers rows already
stored. `yog_core::application::extraction::conformance` states that expectation
once, on a reference mainnet transaction, and every adapter asserts against it.
`transaction_adapter`'s own conformance test is what **pins** that expectation to
reality, by reaching it from the verbatim fixture; the future protobuf adapter
will be checked against reality rather than against itself because of it.

Two suites drive the whole pipeline from the fixtures in `../core/tests/fixtures/`
— read by path, because their value is being the verbatim RPC response and a
second copy would drift. They are unit tests rather than `tests/` targets
because this crate is a binary: an integration target could not reach a
`pub(crate)` adapter without making it public for the tests' sake. The oracle's
witness lives in `testdata/golden/`, not `tests/`, so nothing sits in the
directory Cargo reserves for the targets this crate deliberately does not have.

## Three-stage pipeline

The indexer is structured as three Tokio tasks connected by bounded mpsc
channels. Each stage has a single responsibility, its own typed error channel,
and its own metrics:

```
┌──────────────┐    raw    ┌──────────────────┐  qualified  ┌────────────────┐
│ RpcListener  │──────────▶│ SignatureDispat. │────────────▶│ IndexerWorker  │
│              │  RawLog   │                  │  Signature  │                │
│ logsSubscribe│  Events   │ filter chain:    │  + protocol │ ↓ semaphore-   │
│ + reconnect  │           │ failed / invoc.  │             │   bounded      │
│              │           │                  │             │   spawn        │
└──────────────┘           └──────────────────┘             └────────┬───────┘
                                                                     │
                                                                     ▼
                                                            ┌─────────────────────┐
                                                            │ TransactionProcessor│
                                                            │ fetch (Fetcher) →   │
                                                            │ extract (Dispatcher)│
                                                            │ → persist (Persistor)│
                                                            └─────────────────────┘
```

**`RpcListener`** owns the WebSocket connection, handles reconnection with
exponential backoff, and forwards raw log events downstream. It is itself an
orchestrator of a fleet of `SubscriptionWorker` instances — one per
`SubscriptionTarget`, each with its own retry budget
(`RPC_WORKER_MAX_RETRIES`). Solana's `logsSubscribe` accepts exactly one
pubkey per `mentions` filter, so **what a target is depends on the mode**:

- `INGEST_SCOPE=protocols` — one target per watched protocol, the
  subscription pubkey being the program id. The target mode; it needs an RPC
  that can sustain the full firehose.
- `INGEST_SCOPE=pools` — one target per row of `watched_pools`,
  restored at startup by `WatchedPoolService::restore_subscriptions`. This is
  where the allowlist is enforced: **at the subscription, not by a filter**
  (see [pool observation model](../../README.md#pool-observation-model)).

**`SignatureDispatcher`** applies a chain of filters that turn raw log events
into qualified `(protocol, signature)` pairs. Two filters today: it drops
failed transactions (`FailedTransactionFilter`) and transactions that mention
the program without invoking it — an address-lookup-table reference
(`InvocationFilter`). Signatures that fail to parse are counted separately and
dropped.

**`IndexerWorker`** consumes qualified signatures and drives
`TransactionProcessor` with bounded concurrency. The cap is
`MAX_CONCURRENT_INDEX_TASKS = 15`, calibrated against the Helius free tier
with headroom.

## `TransactionProcessor` and its collaborators

`TransactionProcessor::process(protocol, signature)` composes three
collaborators, each with one responsibility:

- **`TransactionFetcher`** (`infra/rpc/`) — domain-agnostic: knows about RPC
  and retries, not about `Protocol` or event kinds. Classified `FetchError`
  variants; the caller instruments fetch duration with the right `protocol`
  label.
- **`ExtractionDispatcher`** (`yog-core`) — centralises the
  `Protocol → handler` mapping. The indexer never imports concrete extractors;
  adding a protocol updates `yog-core` only.
- **`EventPersistor`** (`application/services/`) — thin dispatcher matching on
  the outer `DomainEvent` variant and delegating to a sub-persistor per
  protocol (`MeteoraDammV2EventPersistor`), which matches on the sub-enum and
  dispatches to per-variant `persist_<kind>` methods against the per-event-kind
  repositories.
- **`PoolMaintenance`** — shared by every sub-persistor via `Arc`. Owns the
  cross-protocol pool registry (`PoolRepository`) and the per-pool projection
  (`PoolCurrentStateRepository`). When a second protocol lands, it reuses the
  same instance.

The wiring happens in `bootstrap/daemon.rs::init_event_persistor` — one of the
two dispatch points a new protocol touches in this crate, the other being
`EventPersistor::persist` above (see the
[add-a-protocol recipe](../README.md#adding-a-new-protocol)).

## Skip-and-log error semantics

- **Per-event failures don't abort the others** — failures from
  `EventPersistor::persist` are logged and counted
  (`yog_indexer_persist_failure_total{event_kind}`), and the next event is
  attempted.
- **A successful insert that wrote nothing is not a success** — every event
  repository returns `InsertOutcome::{Inserted, Skipped}` rather than `()`, so
  an `ON CONFLICT … DO NOTHING` that matched is warned about and counted
  (`yog_indexer_event_insert_skipped_total{event_kind}`) instead of passing for
  a write. Rows actually written are `instructions_indexed − insert_skipped`;
  `instructions_indexed` keeps its meaning, "events processed". On a live
  stream a non-zero skip rate means the unique key is collapsing distinct
  events — the failure that went unseen until the August 2026 audit, when the
  key was `(signature, timestamp)` and discarded the `rows_affected` that would
  have shown it.
- **An order the key cannot decide is counted, not assumed away** — the
  `pool_current_state` projection orders on `(slot, transaction_index,
  event_index)`, but `transaction_index` is empty on the `getTransaction`
  path, so two transactions of one block touching one pool cannot be ranked.
  The repository reports that case and it is counted
  (`yog_indexer_pool_current_state_same_slot_total`), on the applied path as
  well as the rejected one — an ambiguity that wrongly accepts costs as much as
  one that wrongly rejects. The label on the duration histogram is
  `pool_current_state_rejected`, not `stale`: the old name asserted healthy
  concurrency for what was mostly the guard's own second-granularity.
- **Per-signature failures don't stop the worker** — `IndexerWorker` catches
  errors from `process`, logs and counts them, and keeps draining the channel.
- **Loop-level failures bubble up** — closed channels, exhausted semaphores,
  panics in spawned tasks reach `Daemon::run` via typed errors and trigger
  graceful shutdown of all tasks via the shared `CancellationToken`.

An `ExitGuard` RAII helper ensures every entry into `process` produces an exit
counter and duration sample — constructed at the top of the function, mutated
with `guard.set(outcome)` at each exit point; its `Drop` records the metrics,
covering every early return including `?`-propagated errors.

## Observability

Prometheus metrics on `:9000/metrics` (host port `9000` in compose). Every
family carries a `protocol` label; the names below are the ones actually
emitted. No gauges today — all counters and histograms.

- **Pipeline counters** — `yog_indexer_raw_log_events_total`,
  `yog_indexer_raw_log_events_rejected_total{filter, reason}`,
  `yog_indexer_raw_log_events_malformed_total` (unparsable signature),
  `yog_indexer_qualified_signatures_total`,
  `yog_indexer_downstream_saturated_total`
- **Processor counters** —
  `yog_indexer_index_transaction_entered_total`,
  `yog_indexer_index_transaction_exited_total{outcome}`,
  `yog_indexer_transactions_no_match_total`,
  `yog_indexer_fetch_failures_total{reason}`,
  `yog_indexer_fetch_not_found_total`,
  `yog_indexer_unknown_event_total{discriminator}`,
  `yog_indexer_extraction_failure_total{kind}`
- **Persistor counters** —
  `yog_indexer_instructions_indexed_total{instruction}`,
  `yog_indexer_persist_failure_total{event_kind}`,
  `yog_indexer_event_insert_skipped_total{event_kind}`,
  `yog_indexer_pool_current_state_same_slot_total`
- **Histograms** — `yog_indexer_fetch_duration_seconds`,
  `yog_indexer_persist_duration_seconds{kind}`,
  `yog_indexer_index_transaction_duration_seconds{outcome}`

## Configuration

```env
DATABASE_URL_INDEXER=postgresql://yog_indexer:...@localhost:5433/yog_sothoth
SOLANA_RPC_WS=wss://...
SOLANA_RPC_HTTP=https://...
RPC_WORKER_MAX_RETRIES=10
INGEST_SOURCE=rpc
INGEST_SCOPE=pools
```

All six are required — none has an implicit default, and a missing one fails
at startup with a `ConfigError`.

The first three carry a secret and are `SecretUrl`s: userinfo, path, query
string and fragment are redacted in `Display` and `Debug`, while scheme, host
and port stay legible so a failed startup still names what it could not reach.
The path is redacted because providers put credentials there — Alchemy's
`/v2/<key>`, QuickNode's `/<token>/` — and only Postgres URLs keep theirs, it
being the database name.

`SOLANA_RPC_WS` keeps that type all the way down: `RpcListener` clones it once
per worker, and `SubscriptionWorker` exposes it only as the argument of
`PubsubClient::new`. The `inspect_logs` bin reads the same variable through the
same type. The invariant and the guard that enforces it are documented in
`crates/README.md`.

### The two ingestion axes

`INGEST_SOURCE` says **where transactions come from** — the acquisition model,
not the wire protocol: `rpc` notifies then asks (a `logsSubscribe` socket, then
one `getTransaction` per signature), `grpc` delivers (a Yellowstone stream
carrying whole transactions). `INGEST_SCOPE` says **what is subscribed to**
(see above). They are separate variables because they are separate questions,
and all four couples mean something:

| | `INGEST_SCOPE=pools` | `INGEST_SCOPE=protocols` |
|---|---|---|
| **`INGEST_SOURCE=rpc`** | what runs today — the only couple that starts | target mode of the RPC path — **refused** |
| **`INGEST_SOURCE=grpc`** | pool addresses in the subscription filter — **refused** | production target — **refused** |

**Three of the four are refused today**, for two causes, both raised by
`check_supported` in `bootstrap/config/validator.rs`, which `Config::load`
calls **before anything else is read**:

- `grpc`, under either scope, has no listener yet — the RPC path is the only
  implemented source;
- `protocols` builds its targets from `RpcListener::_watch`, which nothing
  calls: the listener would start with zero targets. It gets wired with the
  gRPC migration.

Each refusal is a state of this repository, not a law about the axes: all four
couples are meaningful, and the two `Err` arms disappear together the day that
migration lands. Until then, refusing early is what keeps a configuration
mistake from surfacing as `NoSubscriptionTargets`, which reads like a network
fault and is not one.

Connects to Postgres as `yog_indexer` — RW on event/pool tables, RO on
`watched_pools`.

## Run

```bash
cargo run -p yog-indexer
```
