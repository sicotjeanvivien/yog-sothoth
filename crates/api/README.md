# yog-api

Native binary. HTTP server built on axum — exposes the indexed, enriched, and
detected data as JSON endpoints, plus a Server-Sent Events stream for the live
signal feed. Strictly read-only: it connects under the `yog_api` Postgres role,
which has `SELECT` and nothing else.

For the workspace-level picture (dependency graph, conventions, database
roles, the add-an-endpoint recipe), see [`crates/README.md`](../README.md).

---

## Layout

```
api/src/
├── bootstrap/
│   ├── app_state.rs       ← AppState — dependency container (Arc<dyn Trait>)
│   └── config.rs          ← Config::load() — env-driven
├── application/
│   ├── services/          ← cross-protocol services at the root (PoolService,
│   │   │                    SignalService, StatsService, TokenService,
│   │   │                    NetworkStatusService, AnnouncementService)…
│   │   └── meteora/damm_v2/ …per-protocol ones under their protocol
│   │                        (swap.rs, liquidity.rs) — mirrors core/domain
│   ├── signal_stream.rs   ← SignalStreamPoller (feeds the SSE broadcast)
│   ├── enriched_pool.rs   ← pool + embedded token/price composition
│   └── enriched_signal.rs ← signal + embedded token pair of its pool
├── http/
│   ├── handlers/          ← one module per route family
│   ├── dto/request/       ← query/path DTOs, validated before any DB call
│   ├── dto/response/      ← wire shapes, decoupled from the domain
│   ├── cursor.rs          ← base64/JSON cursor codec
│   ├── query.rs           ← shared query-param validation helpers
│   ├── middleware.rs      ← CORS, security headers, request-id tracing
│   └── error.rs           ← ApiError, IntoResponse (RFC 9457)
└── main.rs
```

Services compose repository reads with cursor encoding and response DTO
mapping; handlers are pure async functions taking axum extractors and
returning `Result<Json<T>, ApiError>`. `AppState` holds every dependency as
`Arc<dyn Trait>` — `Clone` is cheap, and swapping a `Pg*` repository for a
mock in tests is free.

### Where a service goes

A service sits under `services/<protocol>/<product>/` when its repository,
params and result are irreducibly that product's — the swap and liquidity feeds
read columns that exist only in `meteora_damm_v2_*` tables. Everything else stays
at the root, **including services whose only data source today is DAMM v2**.

The test is reach, not current content. `PoolService` serves every protocol's
pools; it reaches per-protocol data through the generic `PoolPropertiesLookup`
and never learns which protocol answered. A cross-protocol service must not name
a protocol in its constructor: that is how a neutral type accretes one
protocol's vocabulary, which is what migration 036 removed from the `pools`
table and what the layout above keeps out of the service layer.

The one place the read path matches on a protocol is
`http/dto/response/pool.rs` — the wire shape genuinely differs per protocol, and
its `PoolProperties` destructuring is irrefutable-by-construction, so adding a
variant breaks the build there instead of silently dropping a field.

## Endpoints

| Method | Path | Description |
|---|---|---|
| `GET` | `/healthz` | Liveness — 200 OK, no DB roundtrip |
| `GET` | `/readyz` | Readiness — pings the DB; 503 with per-check detail when it fails |
| `GET` | `/api/pools` | Paginated list of discovered pools (cursor-based) |
| `GET` | `/api/pools/top` | Top-N pools by `metric` (volume 24h; non-paginated, capped at 20) |
| `GET` | `/api/pools/{address}` | Single pool — everything the list returns, **plus** its protocol's own properties (see below) |
| `GET` | `/api/pools/{address}/latest-state` | Latest observed AMM state for the pool |
| `GET` | `/api/pools/{address}/history` | Hourly time-series buckets (`?days=N`) — volume, fees, liquidity, claims, USD-valued |
| `GET` | `/api/pools/{address}/swap-events` | Paginated swap events |
| `GET` | `/api/pools/{address}/liquidity-events` | Paginated liquidity events |
| `GET` | `/api/network/status` | Latest indexer/RPC slot, RPC latency, observed timestamp |
| `GET` | `/api/signals` | Paginated signal feed (`triggered_at DESC, id DESC`; `?severity=` and `?pool=` filters); each item embeds its pool's token pair (`tokenA`/`tokenB`, same shape as in `PoolResponse`) |
| `GET` | `/api/signals/stream` | SSE stream of new signals (see below) |
| `GET` | `/api/stats` | Global KPIs — total TVL, 24h volume/fees, pool counts |
| `GET` | `/api/tokens/{mint}` | Token metadata + latest price (200 with `price: null` if no price yet) |
| `GET` | `/api/announcements/active` | Operator announcements whose window is open — most severe first, non-paginated (hard cap); empty array is the nominal case |

Public URLs stay protocol-agnostic (`/swap-events`, not `/damm-v2-swaps`); the
service resolves the pool's protocol and reads the matching table.

### Pool response shapes

The three pool endpoints share a base shape and the detail one extends it.

**List and top** (`/api/pools`, `/api/pools/top`) return the cross-protocol
fields only: identity, the token pair, `feeBps`, the analytics block and
`signals24h`. Nothing protocol-specific — one protocol's vocabulary has no place
in every protocol's row.

**Detail** (`/api/pools/{address}`) returns the same fields **flattened at the
top level**, plus one optional block named after the pool's protocol:

```jsonc
{
  "poolAddress": "…", "protocol": "meteora_damm_v2", "feeBps": "25",
  "tokenA": { … }, "tokenB": { … }, "tvlUsd": "…", "signals24h": [ … ],

  // Present only for a DAMM v2 pool that has resolved properties.
  "meteoraDammV2": {
    "protocolFeePercent": 20, "referralFeePercent": 20,
    "baseFeeKind": "constant", "hasDynamicFee": false
  },

  // …and its DLMM sibling, for a `meteora_dlmm` pool. Mutually exclusive with
  // the block above — a pool has one protocol.
  "meteoraDlmm": {
    "binStep": 1, "baseFactor": 10000, "baseFeePowerFactor": 0,
    "variableFeeControl": 2000000, "maxVolatilityAccumulator": 100000,
    "protocolShare": 1000
  }
}
```

Three consequences worth knowing:

- the shared fields are flattened, so **a client holding the list schema parses a
  detail payload** and simply ignores the extra block;
- the block is **absent, not `null`**, when the pool belongs to another protocol.
  A block *present* with `null` fields means the opposite: this protocol's
  satellite exists for the pool but yog-context has not resolved it yet;
- adding a protocol meant adding a sibling field, not changing the existing one —
  and the compiler forced it. The `From<EnrichedPoolDetail>` impl matches
  exhaustively on `PoolProperties`, so a new variant stops the build there rather
  than dropping the block from the wire in silence.

`feeBps` is normalized across protocols and comparable: it is the pool's **base**
fee — the floor a swapper pays before any volatility-driven part — whether that
comes from cp-amm's cliff numerator or DLMM's `baseFactor × binStep`. That is what
lets `/api/pools/fee-tiers` and the `fee_bps` filter span both protocols. The
DLMM inputs are served raw above so a client can recompute it rather than trust
it.

Every pool response — all three endpoints — embeds `signals24h`: the pool's
signals over the last 24h (newest first, capped per pool,
`severity`/`detector`/`triggeredAt` only), which powers the pools-list signal
indicator. One batched query per request (`SignalFeed::recent_by_pools`), not one
per pool.

## The SSE signal stream

`/api/signals/stream` is fed by a single shared **`SignalStreamPoller`**, not
by per-client DB queries:

- One application-level poller ticks every `API_SIGNAL_STREAM_POLL_SECS`
  (default 3 s), reads signals strictly newer than its watermark
  (`SignalFeed::newer_than`), and broadcasts them on a
  `tokio::broadcast` channel to every connected client.
- The watermark is re-anchored to the feed tip on (re)activation — a client
  reconnecting never sees a replay. When `receiver_count() == 0` the DB query
  is skipped and the watermark dropped, so an idle stream costs nothing and a
  returning client gets no burst.
- The handler emits each signal as an SSE event (`data` = the JSON
  `SignalResponse`, `id` = the signal id) with a 15 s keep-alive; a lagged or
  closed receiver ends the stream and the browser's `EventSource` reconnects.
- The poller broadcasts bare `SignalRecord`s; the handler resolves the pool's
  token pair per event at delivery (`SignalService::enrich_one`), so stream
  items carry the same embedded `tokenA`/`tokenB` as the paginated feed. If
  that enrichment fails, the signal is emitted with unresolved sides rather
  than dropped — delivering the alert beats decorating it.

Poller failures are skip-and-log: a failed tick is logged and the next one
proceeds. The poller dies with the process — no dedicated graceful shutdown.

## Error responses

Errors use [RFC 9457 Problem Details](https://www.rfc-editor.org/rfc/rfc9457),
served as `application/problem+json`:

```json
{
  "type": "about:blank",
  "title": "Bad Request",
  "status": 400,
  "detail": "invalid pool address: foo"
}
```

| Status | `title` | Common causes |
|--------|---------|---------------|
| 400 | `Bad Request` | Invalid address, malformed cursor, limit out of range, unknown `severity`/`metric`, mutually exclusive params |
| 404 | `Not Found` | Pool or token unknown, no observed state yet for a known pool |
| 500 | `Internal Server Error` | DB failure, encoding bug. `detail` is always the generic message; the real cause is logged server-side under a `request_id` correlatable via the `x-request-id` response header |

## Cursor wire format

Pagination cursors are **opaque to clients**: base64 (url-safe, no-pad)
encoding of a JSON-serialized `*CursorWire` struct. Clients pass back the
`next_cursor` from the previous response without interpreting it. Default
`limit = 50`, hard cap `200`.

## Configuration

```env
DATABASE_URL_API=postgresql://yog_api:...@localhost:5433/yog_sothoth
API_BIND_ADDR=0.0.0.0:5000
API_CORS_ALLOWED_ORIGINS=http://localhost:3000
API_SIGNAL_STREAM_POLL_SECS=3
```

CORS is locked to the configured dashboard origins — the browser calls this
API directly (there is no BFF in front; see [`web/README.md`](../../web/README.md)).

## Run

```bash
cargo run -p yog-api
curl http://127.0.0.1:5000/healthz
curl http://127.0.0.1:5000/api/pools | jq
```
