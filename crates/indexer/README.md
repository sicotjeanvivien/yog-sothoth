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
│   ├── source.rs          ← the port: TransactionSource + IngestedTransaction
│   ├── services/          ← TransactionProcessor, EventPersistor + the
│   │                        per-protocol sub-persistors (meteora/damm_v2/),
│   │                        PoolMaintenance, WatchedPoolService, metrics
│   ├── reporter/          ← NetworkStatusReporter (Solana slot/latency snapshot)
│   └── workers/           ← IndexerWorker (bounded-concurrency consumer)
├── infra/credential.rs    ← the endpoint's header, validated once, for both paths
├── infra/grpc/            ← Yellowstone: listener, subscription, session,
│                            credential interceptor, protobuf adapter,
│                            slot/time buffer (no source built on it yet)
├── infra/rpc/             ← RpcTransactionSource and the three stages it owns:
│                            RpcListener + SubscriptionWorker (WebSocket fleet),
│                            SignatureDispatcher filter chain, FetchWorker +
│                            TransactionFetcher (HTTP + FetchError)
├── bootstrap/             ← Config::load(), Daemon (composition root: builds
│                            the source, wires the tasks, owns shutdown)
├── error/                 ← typed error per layer
├── bin/inspect_logs.rs    ← ad-hoc debugging helper for raw log streams
└── main.rs
```

## The source adapters

`yog-core` extracts from an `OnChainTransaction` and never learns who filled it.
Filling it is this crate's job, one module per source:

- `infra/rpc/transaction_adapter.rs` turns a `getTransaction` response into that
  neutral shape. It sits beside `transaction_fetcher.rs` on purpose — the
  encoding and the adapter are **one contract** (the fetcher must ask for
  `JsonParsed`, because the adapter reads the `PartiallyDecoded` inner
  instructions only that encoding produces), and splitting them across crates is
  what would let the two drift;
- `infra/grpc/transaction_adapter.rs` turns a Yellowstone
  `SubscribeUpdateTransaction` into the same shape — a sibling module, not a
  second path through extraction.

  Two differences with its JSON-RPC sibling are worth knowing before reading it.
  **The timestamp is an argument**, because `SubscribeUpdateTransaction` carries
  none — that seam is what `slot_timestamp_buffer.rs` below fills. And it
  **filters nothing**: protobuf ships `data` as bytes, so unlike the JSON-RPC
  adapter it has no shape it cannot represent. The same mainnet transaction
  therefore yields 2 payloads through one adapter and 14 through the other,
  which is sanctioned rather than accidental — see *What an adapter owes*.

- `infra/grpc/slot_timestamp_buffer.rs` pairs a transaction with the block time
  its own message does not carry. `block_time` lives on
  `SubscribeUpdateBlockMeta`, a **separate** subscription keyed by slot, while
  `TransactionPosition::timestamp` may not be optional — it is in every event
  table's unique key *and* the partitioning column. So the two streams have to
  be joined, and the wait bounded.

  The bound **counts slots, not seconds**, and the reason is what each choice
  does when things break: a wall clock keeps running while the stream is down,
  so a time bound would empty the buffer during an outage and destroy
  transactions whose block-meta was going to arrive on reconnect. A slot bound
  reads the stream itself — nothing arrives, nothing is evicted. Its default is
  a **ceiling, not an estimate**: the real lag is unmeasured until a live
  stream exists, and `yog_indexer_grpc_untimestamped_transactions_total` is what
  will say whether the ceiling was generous.

- `infra/credential.rs` is **shared by both paths**: it validates the header an
  endpoint declares, once, and hands it to whichever client will carry it — gRPC
  request metadata, or a WebSocket handshake. What decides whether there is a
  header is `INGEST_STREAM_HEADER_NAME` / `_HEADER_VALUE` and nothing else; a
  version that decided on `INGEST_SOURCE` was removed on review, since a
  transport has no business answering a credential question and the belief it
  rested on — that `PubsubClient` cannot send a header — is false.

- `infra/grpc/listener.rs` opens the stream and keeps it open, with
  `subscription.rs` (what is asked for) and `session.rs` (what an update means)
  beside it, and `interceptor.rs` putting the credential on every request. The
  split is by what can be proven without a server: the request and the meaning
  of an update are pure and tested, the connection is neither.

⚠️ **Nothing selects the gRPC path yet.** The listener exists and is complete;
what is missing is a consumer for what it emits and the switch that builds it —
`INGEST_SOURCE=grpc` is still refused at startup by `check_supported`. So
`infra/grpc.rs` still carries a single `#![allow(dead_code)]` for the whole
path, and deleting that one line is part of the switch: the build then names
whatever is still unreachable.

⚠️ And **none of it has met a server.** Every local test is either pure state or
a message this repository built itself, so the connection, TLS, keep-alive, the
retry budget and the exact semantics of `from_slot` are written and reviewed and
unproven. `02 - backlog/pre-v02/flux-grpc-reel-mesures.md` is where they meet
one, and it needs an API key.

**What an adapter owes**, and how it is held to it: the order of the payloads it
produces becomes the persisted `event_index`, part of the unique key of every
event table. An adapter that reorders does not fail — it renumbers rows already
stored. `yog_core::application::extraction::conformance` states that expectation
once, on a reference mainnet transaction, and every adapter asserts against it.
The JSON-RPC adapter's conformance test is what **pins** that expectation to
reality, by reaching it from the verbatim fixture.

⚠️ **The protobuf adapter is not pinned that way, and cannot be.** There is no
mainnet protobuf fixture here and no way to make one without a subscription, so
its inputs are hand-built — the message is constructed with the understanding
the code uses to read it, and the two can agree on a lie. One consequence is
concrete: `program_id_index` resolution walks static keys, then loaded writable,
then loaded readonly, and **no fixture in this repository can witness it** — 25
of the 92 transaction fixtures use address lookup tables, none carries a
`loadedAddresses`, and a JSON-RPC response hands `programId` over already
resolved anyway, so index resolution is structurally a gRPC-only concern. Its
test is built from the documented rule, not from an observation. First
confrontation with reality is the live stream.

`transaction_index` is the one field the two adapters legitimately disagree on —
`getTransaction` omits it, a Yellowstone update always carries it — so
`assert_matches_reference` takes it as an argument and each adapter states what
its source provides. It is also the field this whole migration exists for.

Everything else the two adapters may differ on is bounded by
`InnerInstructionPayload`'s rule: **only ever widen**. Numbering happens after
the filter on the emitting program, so a payload addressed elsewhere costs
nothing to keep or to drop, while dropping one addressed *to* that program
renumbers stored events.

Two suites drive the whole pipeline from the fixtures in `../core/tests/fixtures/`
— read by path, because their value is being the verbatim RPC response and a
second copy would drift. They are unit tests rather than `tests/` targets
because this crate is a binary: an integration target could not reach a
`pub(crate)` adapter without making it public for the tests' sake. The oracle's
witness lives in `testdata/golden/`, not `tests/`, so nothing sits in the
directory Cargo reserves for the targets this crate deliberately does not have.

## One port, and what each source does behind it

The daemon's ingestion graph is **one edge**, whichever acquisition model runs:

```
┌────────────────────────┐  IngestedTransaction  ┌────────────────┐
│ dyn TransactionSource  │──────────────────────▶│ IndexerWorker  │
│  (application/source)  │   bounded, cap 1 000  │ ↓ semaphore-   │
└────────────────────────┘                       │   bounded      │
                                                 └────────┬───────┘
                                                          ▼
                                                 ┌─────────────────────┐
                                                 │ TransactionProcessor│
                                                 │ extract → persist   │
                                                 └─────────────────────┘
```

**Why a port and not a `match`.** The two acquisition models do not differ in
their transport alone: the JSON-RPC path notifies and then *asks*, so it needs
a filter chain and a fetch stage and a concurrency bound set by the RPC quota;
Yellowstone *delivers*, so it needs none of the three. What is identical is
everything below the transaction. The seam therefore belongs where the two
converge — on a translated transaction — and a source owns its whole
sub-graph, however many tasks that takes.

`bootstrap/daemon.rs::init_source` is the only place in the crate that names a
concrete source — nothing downstream of the port learns which one runs.
`INGEST_SOURCE` itself is read in `bootstrap/config.rs` and validated by
`check_supported`; `init_source` does **not** consume it yet, because it has one
arm. Giving it the second arm, and the `match` on the setting, is the next slice.

### The JSON-RPC source (`infra/rpc/source.rs`)

Three stages and two channels, none of which leave the module:

```
RpcListener ──RawLogEvent──▶ SignatureDispatcher ──QualifiedSignature──▶ FetchWorker
 (fleet of WebSockets)        (failed / invocation)                       (getTransaction)
```

**`RpcListener`** owns the WebSocket connections and handles reconnection with
exponential backoff. It is an orchestrator of a fleet of `SubscriptionWorker`
instances — one per `SubscriptionTarget`, each with its own retry budget
(`RPC_WORKER_MAX_RETRIES`). Solana's `logsSubscribe` accepts exactly one pubkey
per `mentions` filter, so **what a target is depends on the mode**:

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

**`FetchWorker`** fetches each surviving signature over HTTP, adapts the
response into the neutral `OnChainTransaction`, and hands it out through the
port. Its cap is `MAX_CONCURRENT_FETCHES = 15`, sized against the Helius free
tier with headroom — **an RPC quota, not a general concurrency setting**, which
is why it lives beside the fetch rather than beside the consumer.

### The consumer's bound, and how an operator changes it

`IndexerWorker`'s cap is **not** a constant: it is the database pool's size
minus the connections this process needs elsewhere (one, for the
`NetworkStatusReporter`), computed at startup by
`bootstrap/daemon.rs::index_concurrency` and logged there. Every task in flight
holds a connection while it persists, so the pool *is* the ceiling — and a pool
too small to reserve from is refused at startup rather than clamped.

With today's pool of 10 the bound is 9. ⚠️ **And it is not configurable**:
`init_db` calls `Database::connect`, whose size is fixed, and no environment
variable reaches `connect_with_options`. Raising the write concurrency therefore
still takes a code change today — what changed is *which* change: sizing the
pool rather than editing a worker constant, with the reservation following
automatically. `index_concurrency`'s startup refusal exists for the pool sizes
that edit could produce, and is covered by tests rather than by a configuration
that can reach it.

### What the two paths cost

| | JSON-RPC | Yellowstone gRPC |
|---|---|---|
| connections | one WebSocket **per target** | one stream |
| per transaction | a second call (`getTransaction`) | nothing — it is delivered |
| the ceiling | requests per second | bandwidth: what you receive and drop is paid for |
| `transaction_index` | absent, so same-slot events cannot be ranked | present |
| filtering | client-side chain | `failed`/`vote` server-side; no invocation equivalent |
| price today | free tier | a subscription, deferred |

The JSON-RPC path stays for that last line: it is the development path with no
monthly cost. ⚠️ **No source is built on the gRPC listener yet** —
`INGEST_SOURCE=grpc` is refused at load time by `check_supported`; the fifth
slice of `03 - active/listener-grpc-yellowstone.md` is what lifts it.

## `TransactionProcessor` and its collaborators

`TransactionProcessor::process_transaction(protocol, &OnChainTransaction)`
composes two collaborators, each with one responsibility. It does **not**
fetch: fetching is what one acquisition model forces on itself, so it belongs
to that source and not to the half of the pipeline both paths share.

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
- **Per-transaction failures don't stop the worker** — `IndexerWorker` catches
  errors from `process_transaction`, logs and counts them, and keeps draining
  the channel. The same rule applies one stage earlier inside the JSON-RPC
  source: a signature the RPC will not return, or a response that will not
  adapt, is counted and stepped over by `FetchWorker`.
- **Loop-level failures bubble up** — closed channels, exhausted semaphores,
  panics in spawned tasks reach `Daemon::run` via typed errors and trigger
  graceful shutdown of all tasks via the shared `CancellationToken`.

An `ExitGuard` RAII helper ensures every entry into `process_transaction`
produces an exit counter and duration sample — constructed at the top of the
function, mutated with `guard.set(outcome)` at each exit point; its `Drop`
records the metrics, covering every early return including `?`-propagated
errors.

⚠️ **It does not span the fetch, and used to.** The consequence is that
`yog_indexer_index_transaction_duration_seconds` measures extract-and-persist
only, and that `..._exited_total` no longer carries `outcome="fetch_not_found"`
or `"fetch_failure"` — those exits happen before a transaction exists and are
counted where they happen, under the unchanged `yog_indexer_fetch_*` names. It
is a deliberate trade: the histogram now measures the same thing on both
acquisition paths, which is what makes comparing them meaningful.

## Observability

Prometheus metrics on `:9000/metrics` (host port `9000` in compose). Every
family carries a `protocol` label; the names below are the ones actually
emitted. No gauges today — all counters and histograms.

- **Pipeline counters** — `yog_indexer_raw_log_events_total`,
  `yog_indexer_raw_log_events_rejected_total{filter, reason}`,
  `yog_indexer_raw_log_events_malformed_total` (unparsable signature),
  `yog_indexer_qualified_signatures_total`,
  `yog_indexer_downstream_saturated_total`
- **Fetch counters** (JSON-RPC source only — there is nothing to fetch on the
  gRPC path, so a series that stops advancing after a source switch is saying
  exactly that) — `yog_indexer_fetch_failures_total{reason}`,
  `yog_indexer_fetch_not_found_total`,
  `yog_indexer_fetch_dropped_total{reason}`. `reason="adapt"` on the failures is
  a response that arrived and could not be turned into an
  `OnChainTransaction`; the *dropped* family is different in kind — work
  discarded rather than work that went wrong. ⚠️ **Its `reason` label separates
  two losses and the total conflates them**: `shutdown` and `downstream_closed`
  cost a request that was made and billed, `shutdown_before_fetch` is a
  signature dropped while queueing for a permit and cost nothing. A non-zero
  `downstream_closed` outside a shutdown means the consumer died first.
- **Worker counter** — `yog_indexer_ingested_dropped_total{reason}`: delivered
  transactions the consumer never processed, `shutdown` for the one in hand and
  `shutdown_queued` for what was still in the channel. It mirrors the fetch
  family one stage down, and exists because the producer was counting its
  shutdown losses while the consumer of the same channel dropped up to a
  thousand more in silence.
- **Processor counters** —
  `yog_indexer_index_transaction_entered_total`,
  `yog_indexer_index_transaction_exited_total{outcome}` — `ok`, `no_events`,
  `extract_failure`, `unknown_exit`,
  `yog_indexer_transactions_no_match_total`,
  `yog_indexer_unknown_event_total{discriminator}`,
  `yog_indexer_extraction_failure_total{kind}`
- **Persistor counters** —
  `yog_indexer_instructions_indexed_total{instruction}`,
  `yog_indexer_persist_failure_total{event_kind}`,
  `yog_indexer_event_insert_skipped_total{event_kind}`,
  `yog_indexer_pool_current_state_same_slot_total`
- **Histograms** — `yog_indexer_fetch_duration_seconds` (JSON-RPC source only),
  `yog_indexer_persist_duration_seconds{kind}`,
  `yog_indexer_index_transaction_duration_seconds{outcome}` — extract and
  persist, **not** the fetch, so the two acquisition paths measure the same
  thing

## Configuration

```env
DATABASE_URL_INDEXER=postgresql://yog_indexer:...@localhost:5433/yog_sothoth
INGEST_STREAM_URL=wss://...            # + INGEST_STREAM_KEY if it has a {key}
INGEST_TRANSACTION_URL=https://...     # + INGEST_TRANSACTION_KEY likewise
RPC_WORKER_MAX_RETRIES=10
INGEST_SOURCE=rpc
INGEST_SCOPE=pools
```

All six are required — none has an implicit default, and a missing one fails
at startup with a `ConfigError`.

**Two endpoints, four variables.** Each is an `Endpoint`: a `<FUNCTION>_URL`
carrying `{key}` where the provider expects its credential, plus a
`<FUNCTION>_KEY` holding it. They are named after what they serve — what the
ingestion listens to, and where a transaction is fetched back from — never
after the protocol they speak. The variable they replace, `SOLANA_RPC_HTTP`,
was named for its transport, so it excluded nothing and had accumulated three
roles across two crates; one variable cannot hold two addresses, which is the
wall a provider migration would have hit. A URL with no `{key}` and no key is a
public endpoint and is used verbatim; the two mismatches — a `{key}` without
its key, a key without its `{key}` — are refused at startup, naming the
variable. See `crates/README.md` for the type, and `.env.example` for the
convention.

`DATABASE_URL_INDEXER` carries its secret *inside* the URL, because `sqlx`
wants the string whole, and is a `SecretUrl`: userinfo, path, query string and
fragment are redacted in `Display` and `Debug`, while scheme, host and port
stay legible so a failed startup still names what it could not reach. The path
is redacted because providers put credentials there — Alchemy's `/v2/<key>`,
QuickNode's `/<token>/` — and only Postgres URLs keep theirs, it being the
database name.

An assembled endpoint is a `SecretUrl` too, and keeps that type all the way
down: `Endpoint::url()` builds one, `RpcListener` clones it once per worker,
and `SubscriptionWorker` exposes it only as the argument of `PubsubClient::new`.
The `inspect_logs` bin reads the same pair through the same types. The
invariant and the guard that enforces it are documented in `crates/README.md`.

### Scrubbing what a third party wrote

`Display` protects the URL while we hold it. It does nothing for an error
*somebody else* built: measured on 4 September 2026 by driving `RpcClient` at an
unresolvable host, `reqwest` renders `error sending request for url
(https://…/v2/<key>)` and `solana-client` passes it through untouched. A
WebSocket failure, by contrast, says only `unable to connect to server` — it
carries no URL at all.

`SecretUrl::scrub` removes that. It runs **where the third party's string is
born**, not where it is later logged: `TransactionFetcher` and
`NetworkStatusReporter` hold the endpoint for that single purpose, and
`SubscriptionWorker` already had it. Nothing carrying a URL reaches a `warn!`,
so no log site has a rule to remember — the placement `yog-context` arrived at
in `error/source.rs`, after nine call sites each recopied an unredacted error
and leaked its API key into 38 log lines.

It replaces `utils/redact.rs`, deleted here, whose `redact_api_key` searched for
the literal `api-key=`. That function could not see a credential in a path, and
teaching it that shape would have added a fourth thing to recognise to a
redactor whose whole weakness was having to recognise anything. `scrub` knows
its **own** secret instead, so a provider that hides a credential somewhere new
is covered the day the configuration points at it.

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

- `grpc`, under either scope, has a listener but nothing that selects it: what
  it emits has no consumer, and `init_source` has one arm. Its refusal is
  therefore narrower than it was, and it still holds;
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
