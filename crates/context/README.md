# yog-context

Native binary. Enrichment daemon — complements the raw on-chain data recorded
by the indexer with what the event stream alone cannot provide: token
metadata, USD prices, and pool account properties.

For the workspace-level picture (dependency graph, conventions, database
roles), see [`crates/README.md`](../README.md).

---

## Layout

```
context/src/
├── source/       ← ports: MetadataSource, PriceSource, PoolAccountSource
├── providers/    ← adapters: HeliusDasClient, JupiterPriceClient,
│                   SolanaAccountClient (+ provider metrics)
├── workers/      ← use cases: MetadataWorker, PriceWorker, PoolAccountWorker
│                   (+ per-worker metrics)
├── bootstrap/    ← Config::load(), Daemon::new — composition root
├── error/        ← SourceError, WorkerError
└── main.rs
```

The ports/providers split keeps the workers testable: a worker depends on a
`source` trait, never on the concrete HTTP client. Providers chunk and fetch
internally; a worker makes a single `fetch_*` call per tick and upserts what
came back.

Providers are **transport only** — they do not interpret payloads. The account
source returns raw bytes plus the owner; base64 is the RPC's encoding, so it is
decoded at this boundary and never reaches `core`, which stays free of it.

## Three workers, two cadences

- **`MetadataWorker`** — every `CONTEXT_METADATA_POLL_SECS` (default 10 s),
  queries `TokenMetadataRepository::list_missing_mints` for mints present in
  `pools` but absent from `token_metadata`, and fetches symbol / name /
  decimals / logo via Helius DAS.
- **`PriceWorker`** — every `CONTEXT_PRICE_INTERVAL_SECS` (default 30 s),
  lists the known mints and asks Jupiter Price V3 for current USD prices,
  inserting them with a single shared `fetched_at` per tick.
- **`PoolAccountWorker`** — same cadence as the metadata worker. Fills every
  account-derived pool property: mints, base fee, fee split, fee shape.

  **It is the only writer of those columns.** The indexer does not decode
  property values from events — it raises `pools.needs_refresh` when an event
  changes one, and this worker re-reads the account. So the queue has two
  entries: pools never resolved (a NULL column) and pools flagged stale. That is
  what lets a one-shot back-fill track values that move, without polling every
  pool on a timer.

  Reading the account rather than an update event is also what removes a class
  of decoding hazard: an account carries resolved state at fixed offsets, an
  update carries a delta with variable-offset borsh tags and `Option`s that
  encode three states.

  ⚠️ Worth knowing, because the crate's name does not suggest it: **yog-context
  reads on-chain pool *accounts*, not just token metadata and prices.** That is
  what this worker is.

  It **names no protocol**. It holds one `PoolAccountResolver` per protocol —
  each owning its own queue and its own satellite — plus the `PoolRepository`
  for the shared registry and the shared `SolanaAccountClient`, and iterates.
  Each decoded account is written in two halves, **satellite first, registry
  last**: the registry write is what lowers the refresh flag, so a failure
  before it leaves the pool queued rather than half-refreshed. Decoding happens in
  `yog_core::application::decode_pool_account`, routed on the account's owning
  program id (what the chain calls its `owner`), so one client serves every
  protocol. Adding one means pushing a resolver into the vec in `bootstrap`;
  not a line of the worker changes — **verified**, not asserted: DLMM was added
  in exactly one line here (its satellite is baseline §9) and this worker was
  untouched.

  ⚠️ The cost of that genericity falls on the resolver: each one's
  `list_unresolved` **must** filter on its own protocol. A per-protocol satellite
  table does not scope the query by itself, because "has no satellite row yet" is
  one of the conditions that makes a pool a candidate — and that is permanently
  true of every pool of every *other* protocol. Get it wrong and the queue
  proposes pools this resolver can never store, so they never leave it, and with
  `ORDER BY first_seen_at` and a capped batch they pile up at the head and starve
  enrichment for everything behind them. Covered in both directions by
  `persistence/tests/pool_properties.rs`.

## Resilience contract

All workers are **deliberately resilient**: HTTP errors, decode errors, and
per-row persistence errors are absorbed inside the loop (logged and counted,
then `continue`). An `Err` returned from a source trait is reserved for
structural misconfiguration, not partial fetch failures — those are handled
internally as skip-and-log per chunk.

One refinement on the Jupiter side: chunks are sent back-to-back, so a tick
with many mints can trip Jupiter's rate limit and 429 the later chunks. The
client retries a rate-limited chunk a bounded number of times (pacing on the
`Retry-After` header when present, capped exponential backoff otherwise)
before falling back to skip-and-log.

There is deliberately no in-process respawn logic: a worker never returns
`Err` from its loop, and a panic exits the whole process, which the container
restart policy relaunches with a fresh budget. The failure mode that policy
cannot see — a provider call hanging forever with the process still alive —
is closed by the shared `providers::http_client()`: every provider client
carries a total-request timeout (15 s) and a connect timeout (5 s), so a hang
degrades into a tick-level `SourceError` absorbed like any other.

## Observability

Prometheus metrics on `:9000/metrics` (host port `9001` in compose): per-worker
tick/upsert/failure counters and per-provider request counters and durations.

## Configuration

```env
DATABASE_URL_CONTEXT=postgresql://yog_context:...@localhost:5433/yog_sothoth
SOLANA_RPC_HTTP=https://mainnet.helius-rpc.com/?api-key=...
JUPITER_URL=https://api.jup.ag
JUPITER_API_KEY=...
CONTEXT_METADATA_POLL_SECS=10
CONTEXT_PRICE_INTERVAL_SECS=30
```

Connects to Postgres as `yog_context` — RW on `token_metadata` and
`token_prices`, `UPDATE` on the pool-property columns of `pools`, RO
otherwise.

## Run

```bash
cargo run -p yog-context
```
