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
│   ├── services/          ← one service per resource: PoolService,
│   │                        SignalService, StatsService, TokenService,
│   │                        NetworkStatusService, MeteoraDammV2Swap/Liquidity
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

## Endpoints

| Method | Path | Description |
|---|---|---|
| `GET` | `/healthz` | Liveness — 200 OK, no DB roundtrip |
| `GET` | `/readyz` | Readiness — pings the DB; 503 with per-check detail when it fails |
| `GET` | `/api/pools` | Paginated list of discovered pools (cursor-based) |
| `GET` | `/api/pools/top` | Top-N pools by `metric` (volume 24h; non-paginated, capped at 20) |
| `GET` | `/api/pools/{address}` | Single pool, enriched with token metadata, prices, analytics |

Every pool response (the three endpoints above) also embeds `signals24h`: the pool's signals over the last 24h (newest first, capped per pool, `severity`/`detector`/`triggeredAt` only) — the pools-list signal indicator. One batched query per request (`SignalFeed::recent_by_pools`), not one per pool.
| `GET` | `/api/pools/{address}/latest-state` | Latest observed AMM state for the pool |
| `GET` | `/api/pools/{address}/history` | Hourly time-series buckets (`?days=N`) — volume, fees, liquidity, claims, USD-valued |
| `GET` | `/api/pools/{address}/swap-events` | Paginated swap events |
| `GET` | `/api/pools/{address}/liquidity-events` | Paginated liquidity events |
| `GET` | `/api/network/status` | Latest indexer/RPC slot, RPC latency, observed timestamp |
| `GET` | `/api/signals` | Paginated signal feed (`triggered_at DESC, id DESC`; `?severity=` and `?pool=` filters); each item embeds its pool's token pair (`tokenA`/`tokenB`, same shape as in `PoolResponse`) |
| `GET` | `/api/signals/stream` | SSE stream of new signals (see below) |
| `GET` | `/api/stats` | Global KPIs — total TVL, 24h volume/fees, pool counts |
| `GET` | `/api/tokens/{mint}` | Token metadata + latest price (200 with `price: null` if no price yet) |

Public URLs stay protocol-agnostic (`/swap-events`, not `/damm-v2-swaps`); the
service resolves the pool's protocol and reads the matching table.

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
