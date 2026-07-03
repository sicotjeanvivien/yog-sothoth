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
│              │           │ / watched_pool   │             │   spawn        │
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
orchestrator of a fleet of `SubscriptionWorker` instances (one per pool in the
allowlist), each with its own retry budget (`RPC_WORKER_MAX_RETRIES`).

**`SignatureDispatcher`** applies a chain of filters that turn raw log events
into qualified `(protocol, signature)` pairs — drops failed transactions,
transactions that don't actually invoke the watched protocol, and
(temporarily) transactions outside the watched-pool allowlist.

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
three dispatch points a new protocol touches (see the
[add-a-protocol recipe](../README.md#adding-a-new-protocol)).

## Skip-and-log error semantics

- **Per-event failures don't abort the others** — failures from
  `EventPersistor::persist` are logged and counted
  (`persist_failures_total{event_kind}`), and the next event is attempted.
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

Prometheus metrics on `:9000/metrics` (host port `9000` in compose). Key
families:

- **Pipeline counters** — `raw_log_events_total`,
  `raw_log_events_rejected_total{filter, reason}`, `qualified_signatures_total`,
  `downstream_saturated_total`
- **Processor counters** — `index_transaction_entered/exited_total{outcome}`,
  `transactions_no_match_total`, `unknown_event_total{discriminator}`,
  `extraction_failure_total{kind}`, `fetch_failures_total{reason}`,
  `fetch_not_found_total`
- **Persistor counters** — `instructions_indexed_total{protocol, instruction}`,
  `persist_failures_total{protocol, event_kind}`
- **Allowlist filter** — `watched_pool_filter_passed_total{pool_address}`,
  `watched_pool_filter_dropped_total`
- **Histograms** — `fetch_duration_seconds`,
  `persist_duration_seconds{protocol, kind}`,
  `index_transaction_duration_seconds{outcome}`
- **Gauges** — `watched_pools_active`

## Configuration

```env
DATABASE_URL_INDEXER=postgresql://yog_indexer:...@localhost:5433/yog_sothoth
SOLANA_RPC_WS=wss://...
SOLANA_RPC_HTTP=https://...
RPC_WORKER_MAX_RETRIES=10
MODE_PROTOCOL_CENTRIC=true
```

Connects to Postgres as `yog_indexer` — RW on event/pool tables, RO on
`watched_pools`.

## Run

```bash
cargo run -p yog-indexer
```
