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
│                   CpammPoolClient (+ provider metrics)
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

## Three workers, two cadences

- **`MetadataWorker`** — every `CONTEXT_METADATA_POLL_SECS` (default 10 s),
  queries `TokenMetadataRepository::list_missing_mints` for mints present in
  `pools` but absent from `token_metadata`, and fetches symbol / name /
  decimals / logo via Helius DAS.
- **`PriceWorker`** — every `CONTEXT_PRICE_INTERVAL_SECS` (default 30 s),
  lists the known mints and asks Jupiter Price V3 for current USD prices,
  inserting them with a single shared `fetched_at` per tick.
- **`PoolAccountWorker`** — same cadence as the metadata worker. Backfills the
  nullable pool properties that events alone leave NULL for pools created
  before the indexer started: it reads the cp-amm `Pool` account directly and
  resolves the **mints**, the **base fee** (`cliff_fee_numerator`, u64 at
  offset 8 → `fee_bps`), and the **fee split**
  (`protocol/partner/referral_fee_percent`, u8 at offsets 48/49/50). The
  event-driven paths in the indexer remain the live refreshers; this worker is
  the catch-up for pre-existing pools.

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

Known limitation (tracked for the public release): a worker whose retry budget
is exhausted currently stays down until the process restarts — there is no
respawn logic yet.

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
