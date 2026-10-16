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
│   ├── app_state.rs       ← build_app_state() — opens the pool, wires the repositories
│   │                        into the services, and builds the signal broadcast with the
│   │                        poller it returns, the stream slots and the work slots
│   ├── config.rs          ← Config::load() — env-driven
│   └── serve.rs           ← serve() — the server and the poller, and their stop
├── application/
│   ├── services/          ← cross-protocol services at the root (PoolService,
│   │   │                    SignalService, StatsService, TokenService,
│   │   │                    NetworkStatusService, AnnouncementService)…
│   │   └── meteora/damm_v2/ …per-protocol ones under their protocol
│   │                        (swap.rs, liquidity.rs) — mirrors core/domain
│   ├── signal_stream.rs   ← SignalStreamPoller (enriches once, feeds the SSE broadcast)
│   ├── cache.rs           ← SharedCache (moka) and its named CachePolicy per use
│   ├── enriched_pool.rs   ← pool + embedded token composition
│   └── enriched_signal.rs ← signal + embedded token pair of its pool
├── http/
│   ├── handlers/          ← one module per route family
│   ├── dto/request/       ← query/path DTOs, validated before any DB call
│   ├── dto/response/      ← wire shapes, decoupled from the domain
│   ├── dto/schema.rs      ← the wire forms the contract names (UnsignedDecimal, Count…)
│   ├── tests/contract.rs  ← test-only: the OpenAPI document, as committed in openapi.json
│   ├── cursor.rs          ← base64/JSON cursor codec
│   ├── query.rs           ← shared query-param validation helpers
│   ├── middleware.rs      ← CORS, security headers, request-id tracing,
│   │                        capacity bounds (middleware/capacity.rs)
│   ├── state.rs           ← AppState — what every handler reads through `State`
│   └── error.rs           ← ApiError, IntoResponse (RFC 9457, 503 + Retry-After)
├── testing.rs             ← test-only: fixtures and repository mocks
└── main.rs
```

Services compose repository reads with cursor encoding and response DTO
mapping; handlers are pure async functions taking axum extractors and
returning `Result<Json<T>, ApiError>`. `AppState` holds the services and the
few runtime handles the router and the handlers read (signal broadcast, stream
and work slots, health probe, stop token) — `Clone` is cheap. The services take repositories as
`Arc<dyn Trait>`, so a service test swaps a `Pg*` repository for a mock from
`testing.rs`.

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
`http/dto/response/pool_detail.rs` — the wire shape genuinely differs per protocol, and
its `PoolProperties` `match` is exhaustive with no wildcard arm, so adding a
variant breaks the build there instead of silently dropping a field.

### Where a response type goes

`http/dto/response/` follows one rule throughout:

- **Name.** Every type ends in `Response`. An enum takes the name of the domain
  type it translates (`domain::FreshnessStatus` → `FreshnessStatusResponse`),
  so the two never share a name — without the protocol prefix when its response
  has none (`LiquidityEventKindResponse` beside `LiquidityEventResponse`). `every_schema_is_a_response_or_a_wire_form`
  fails on a schema of the contract named otherwise.
- **Place.** One type per file. At the top level, what a route returns — its
  body, or the item of a page. A part of a response lives in that response's
  folder, one level deep; its file drops the owner's prefix, its type keeps it
  (`announcement/kind_response.rs` holds `AnnouncementKindResponse`). A part several
  responses share lives with the one it is about, and the others reach it
  through it: `pool/protocol_response.rs`, `signal/severity_response.rs`,
  `token/embedded_token_response.rs`.
- **Visibility.** Each item and field declares the reach it has. A field is
  private unless something outside its file builds or reads it — the handlers
  build `PageResponse`, `PoolPageResponse` and the readiness types. `pub(super)`
  is for what only its folder and that folder's tests use, `pub(in
  crate::http::dto::response)` for a part other responses reach, `pub(crate)`
  for what the handlers see.
- **Conversions** live with the type they build, and a behaviour two responses
  share is written once (`logo_uri` in `token.rs`).
- **Tests** of a part go in its owner's file, `response/tests/<owner>_tests.rs`.

## Endpoints

| Method | Path | Description |
|---|---|---|
| `GET` | `/healthz` | Liveness — 200 OK, no DB roundtrip |
| `GET` | `/readyz` | Readiness — pings the DB; 503 with per-check detail when it fails |
| `GET` | `/api/pools` | Paginated list of discovered pools (cursor-based) — filters and sort below |
| `GET` | `/api/pools/fee-tiers` | The fee tiers worth offering as a filter: the top-8 `fee_bps` values by pool count, each with its count, re-ordered ascending for display (non-paginated) |
| `GET` | `/api/pools/top` | Top-N pools by `metric` — `volume_24h` (default) or `tvl`; non-paginated, default 10, capped at 20 |
| `GET` | `/api/pools/{address}` | Single pool — everything the list returns, **plus** its protocol's own properties (see below) |
| `GET` | `/api/pools/{address}/latest-state` | Latest observed AMM state for the pool, and the USD value of each side under the TVL's 15-minute price bound |
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

### Filtering and sorting the pool list

`GET /api/pools` is the one endpoint whose SQL shape varies with the request —
hence the lone `QueryBuilder` in `persistence` (`repositories/pool/query.rs`),
covered by integration tests since the macros cannot check it.

| Param | Accepted values | Semantics |
|---|---|---|
| `q` | free text, or `X/Y` | With a `/` it is a **pair** filter: the pool must hold one token matching each side, on the two distinct mint columns (`SOL/USDC` finds the pool either way round). Without one, a single term matched against the pool address (exact) or either token's symbol/name (`ILIKE`). |
| `fee_bps` | a decimal string, e.g. `25` | Exact match on the pool's base fee. Meant to be fed from `/api/pools/fee-tiers`, but bound like any user value. |
| `sort` | `last_seen_desc` (default), `last_seen_asc`, `first_seen_desc`, `first_seen_asc` | Ordering, always tie-broken by `pool_address`. The `last_seen_*` pair sorts on a **mutable** column — see the snapshot anchor below. |
| `cursor` / `dir` / `position` / `limit` | see below | Standard cursor pagination. |

The cursor is stamped with the sort column it was built for; passing it back
under a different `sort` is a 400 rather than a silently wrong page.

#### Sorting on `last_seen_at` — the traversal is a snapshot

`pools.last_seen_at` is rewritten on every event touching the pool, and a
keyset cursor assumes its sort key holds still: a row whose value moves crosses
to the other side of the cursor. So a `last_seen_*` traversal is **anchored**:
the first page mints `asOf`, every cursor carries it, and each page reads only
`last_seen_at <= asOf`. Since the column only grows, the result set can only
shrink — a pool touched mid-traversal leaves it rather than moving inside it.

Two fields on the response say so, in addition to the shared envelope:

| Field | Meaning |
|---|---|
| `asOf` | The instant the traversal is anchored to, RFC 3339. `null` on a `first_seen_*` sort — an immutable column needs no anchor. |
| `touchedSince` | How many pools **matching the same `q` / `fee_bps`** became active after `asOf`. `0` whenever the anchor was minted by that same call — a first page, or a `position` jump — but **not** on a backward page that lands back on the first page, which carries a cursor and so a real count. |

⚠️ `touchedSince` is an **upper bound on what the reader missed, not a count of
it**: a pool they already saw on an earlier page and which was touched again is
counted too. Once a pool is touched its previous `last_seen_at` is gone, so
nothing can distinguish the two after the fact. Surface it as "became active
since `asOf`", never as "pools you missed".

⚠️ And it is a bound on **movement**, not on membership. With `fee_bps` or `q`
active, a pool can enter the filtered set mid-traversal without being touched —
`yog-context` resolving its properties writes `fee_bps` without moving
`last_seen_at`. Such a pool is neither shown nor counted, and no count keyed on
`last_seen_at` can catch it: nothing records when a row started matching a
filter.

A cursor's `asOf` is validated like every other field it carries. One in the
future would make the anchor match everything — the traversal silently
unanchored while `touchedSince` reports `0` — and one older than an hour is a
bookmarked URL rather than a live traversal, replaying a stale snapshot.

**A refused `asOf` costs the cursor its place too**, and the listing restarts at
the head: a fresh anchor over the old cursor position would reintroduce, for one
hop, exactly the bug the anchor exists to prevent. `isFirst: true` on the
response is how a client tells that this happened. A cursor carrying **no**
`asOf` at all is the one exception — it was minted before the field existed, so
it keeps its place and simply gets an anchor.

The **first page of a traversal is served unfenced**: with no cursor there is no
keyset assumption to protect, and bounding it could only hide rows. The anchor
is minted there all the same and stamped into the outgoing cursors, so every
page after it is fenced.

What this does and does not buy: under `last_seen_asc` the duplicate is gone,
and backward navigation no longer pulls in rows that moved. Under
`last_seen_desc` a pool touched mid-traversal still isn't shown by that
traversal — it now belongs at the head of the list, which the reader has
already passed — but it is counted in `touchedSince` instead of vanishing
silently. Clients are expected to surface that count and offer a return to the
first page; the dashboard does.

A cursor minted before `asOf` existed still decodes (the field is optional):
the traversal simply re-anchors on the next page rather than 400-ing a URL that
was valid when it was issued.

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
    "baseFeeKind": "constant", "hasDynamicFee": false,
    "currentFeeBps": null, "feeSchedulerExpired": null
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

`feeBps` is normalized across protocols: it is the pool's **genesis base fee**,
whether that comes from cp-amm's cliff numerator or DLMM's
`baseFactor × binStep`. That is what lets `/api/pools/fee-tiers` and the
`fee_bps` filter span both protocols.

⚠️ **It is a floor for some pools and a ceiling for others**, and the difference
is not cosmetic. On DLMM, and on a cp-amm pool with a constant fee or a rate
limiter, it is the floor a swapper pays before any volatility-driven part. On a
cp-amm **fee scheduler** — 34 % of the pools observed — it is the fee at period
0 of a *decreasing* curve, so it is the **maximum**, and measured against
Meteora's own API it was off by ×5 and ×49 on two live pools. `currentFeeBps` in
the `meteoraDammV2` block is that curve evaluated at read time; it is `null`
whenever it cannot be established honestly (no time curve, unresolved account,
or a slot-activated pool). `effectiveFeeBps` — realized fees over realized
volume — is the other honest number, and needs no decoding at all.

⚠️ **Same definition, different upper bound.** Two pools at the same `feeBps`
are not interchangeable: a DLMM pool's variable fee is bounded by its own
parameters, and on real mainnet accounts that bound runs from ×1 (no dynamic
fee) to **×7 the floor**. A client that needs the worst case can compute it —
which is why `variableFeeControl`, `maxVolatilityAccumulator` and `binStep` are
served raw above rather than folded into `feeBps`:

```text
maxVariableFeeBps = ⌈variableFeeControl × (maxVolatilityAccumulator × binStep)² / 1e11⌉ / 1e5
```

Worked examples and the derivation are in `yog_core::amm::dlmm::base_fee_bps`.

Every pool response — all three endpoints — embeds `signals24h`: the pool's
signals over the last 24h (newest first, capped per pool,
`severity`/`detector`/`triggeredAt` only), which powers the pools-list signal
indicator. One batched query per request (`SignalFeed::recent_by_pools`), not one
per pool.

## What a route may return

These rules hold for every `/api` route. Some current routes predate them
(`/api/stats`, the pool routes); each changes with the page that needs it.

1. **A route names a resource of the data.** Its path is a noun phrase:
   `/api/<collection>`, `/api/<collection>/<id>`,
   `/api/<collection>/<id>/<sub-resource>`. Collections are the names of the
   domain and of the analytic model: protocols, pools, tokens, signals, events,
   announcements.
2. **Every field of a response completes the name of its route.** The route
   reads as a sentence ("the activity of the DAMM v2 protocol over 24 h") and
   each field is an attribute of it. A field that needs another name to describe
   itself belongs to another route. A response embeds the *identity* of another
   resource (the symbol and logo of a signal's tokens), nothing else: neither
   its state nor its measures.
3. **A response does not mix what is read live and what `yog-analytic`
   computes.**
   - Live: the catalogue (pools, tokens, protocols), the current state
     (reserves, price, a pool's TVL), events, signals — written by the indexer
     and `yog-context`, fresh to the second.
   - Computed: every measure over a window (volume, fees, swaps, active pools,
     births, the TVL of a protocol or a token and its history) — written by
     `yog-analytic`, fresh to a few minutes.
   - Mixed, the slower makes the other unavailable, and a response can no longer
     state the age of what it holds.
   - The boundary is the nature of the data, not today's cost: a measure over a
     window is computed, even if it would count fast live.
   - A computed response embeds the identity of what it measures (a ranking by
     volume carries each pool's address and tokens): an identity does not age
     and costs nothing. The reverse is forbidden.
4. **A measure is read from `yog-analytic`'s tables**, never from a view that
   values at read time. That is what holds the delay whatever the window.
5. **Measures have one shape**: how many X, over the window W, grouped by G.
   - The subject is in the path, the window in a parameter:
     `/api/protocols/{protocol}/activity?window=24h` for the totals,
     `/api/protocols/{protocol}/activity/hourly?window=24h` for the series. A
     collection returns one row per subject: `/api/tokens/activity?window=24h`.
   - TVL has its own routes on the same model: `/api/protocols/{protocol}/tvl`,
     `/api/protocols/{protocol}/tvl/hourly`.
   - A measure's response carries its window, the instant it reflects, and its
     coverage.
6. **What belongs to one protocol is named after it.** The shared resources
   (pools, tokens, signals) carry the protocol as a field and accept it as a
   filter. What exists in one protocol only goes in a block named after it (the
   pool detail's rule, below) or under `/api/protocols/{protocol}/…`.
7. **The form rules stay as they are** — cursor pagination, Problem Details,
   camelCase, RFC 3339 — written once, in the contract.
8. **The contract is written once, on the server, and the client derives from
   it**: an OpenAPI document drawn from the Rust types, from which the web's zod
   is generated ([The contract](#the-contract)).

**Names.** `activity` for the flows over a window (volume, fees, swaps,
liquidity, claims, rewards, active pools, births), `tvl` for the value
deposited. A series is a sub-resource, `hourly` then `daily`: a parameter never
changes the shape of a response. `window=24h|7d|30d`, a closed list, 400
otherwise; `offset` for the previous window; `asOf` for the instant a response
reflects. The protocol is written `meteora_damm_v2` everywhere (field, filter,
path); paths in kebab-case, collections plural, fields camelCase. No catch-all
name (`analytics`, `metrics`, `stats`): a category name excludes nothing.

**Delay.** Every route but the probes and the live stream answers in under
**2 s** in production, network included, at the heaviest its parameters allow,
cold and warm. A route above 2 s in production is a bug.

## The contract

Every `/api` route is mounted by `api_routes` (`http.rs`) with
`.routes(routes!(handler))`, and declared on its handler by `#[utoipa::path]`:
path, parameters, 200 body. Its DTOs derive `ToSchema`. The OpenAPI document
read from that list is committed as [`openapi.json`](./openapi.json), and the
web generates its zod from it — see
[`web/README.md`](../../web/README.md#the-api-contract). The API does not serve
it.

What the derive cannot say is said once:

- **Named wire forms** (`http/dto/schema.rs`): `UnsignedDecimal`,
  `SignedDecimal`, `U128String`, `Count`. A field points at one with
  `#[schema(value_type = …)]`. A `Decimal` has no default form — utoipa's
  `decimal` feature is off — so a `Decimal` field does not compile until it
  names one, signed or not.
- **Closed sets** are API enums built from their domain enum by an exhaustive
  `From`: a variant added to the domain breaks the build there, and a test
  checks that each serialises as the domain's `as_str()`.
- **The finish** (`http/tests/contract.rs`), applied to the whole document: a nullable
  property is required — `null` is written, never omitted, and a key that may be
  absent says `#[schema(nullable = false)]` (a test fails on a
  `skip_serializing_if` without it); every operation declares its failures —
  Problem Details, and plain text too on an operation with parameters, which
  axum refuses that way when it cannot parse them; an `int64` (or `u64`) anywhere fails the document, because the generated
  zod would read it as a `BigInt` (point it at `Count`); so does a `$ref` to an
  undeclared schema, which the generator would skip in silence.
- **Integers carry their Rust range**: utoipa's `non_strict_integers` writes a
  `u8`, `u16` or `u32` with its own format (`u-int8`…), never a capped `int32`,
  and the finish gives it the maximum of its width unless it declares a lower one.
- **Refinements** the Rust type does not carry: `min_length = 1`,
  `maximum = 100`, `format = "uri"`. A bound the server validates (`limit`,
  `days`) is written in its `#[param]`, and a test pins it to the constant.

After changing a route or a DTO, regenerate both, and commit them:

```bash
YOG_WRITE_OPENAPI=1 cargo test -p yog-api contract   # rewrites crates/api/openapi.json
cd web && npm run gen:api                             # rewrites web/src/lib/api/schema/
```

`cargo test` fails while `openapi.json` is stale, and the CI `contract` job
regenerates the zod from it and fails on any difference with what is committed. A route
mounted with a literal path — `.route("/api/…")` serves it without a contract —
fails `every_api_route_is_mounted_with_its_contract`.

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
- The poller resolves each tick's token pairs **once**, in one batch
  (`SignalService::enrich_batch`, the same code as the paginated feed), and
  broadcasts the enriched signals behind an `Arc`: a stream does no database
  work. Until September 2026 each stream enriched every signal itself, so N
  open streams turned one signal into N rounds of lookups on a 10-connection
  pool. If the enrichment fails, the signals go out with unresolved sides
  rather than being dropped — delivering the alert beats decorating it.
- At most `SSE_MAX_STREAMS` (200) streams are open at once; the next one is
  refused with `503`. See [Capacity bounds](#capacity-bounds).

Poller failures are skip-and-log: a failed tick is logged and the next one
proceeds.

## Stopping

SIGTERM or Ctrl-C (`yog_bootstrap::shutdown_signal`) cancels one token, and
`bootstrap::serve` stops the process the way the daemons stop theirs: the
server with axum's `with_graceful_shutdown`, then the poller, both waited on
by `yog_bootstrap::Stop` within `SHUTDOWN_GRACE`, which names whichever
outlives it.

⚠️ **A graceful shutdown alone would not stop this server.** It stops
accepting, then waits for every open connection — and an SSE connection never
ends on its own. The stream reads the same token (`signal_sse`, `take_until`)
and closes when it fires; the browser's `EventSource` reconnects to the next
instance by itself. Any other request in flight is answered before `serve`
returns: once it does, `main` returns and the runtime takes every connection
task with it.

Before this, the binary installed no signal handler at all. In its container it
is PID 1, and the kernel does not apply a signal's default action to PID 1, so
SIGTERM was ignored and Docker killed it ten seconds later (`Exited (137)`).

## Capacity bounds

One client must not be able to take the API down for everyone. Measured on
25 September 2026, before these bounds, with the dev database: 30 parallel
`/api/pools/top` requests answered `500` ten times and made a 4 ms route
answer `500` after 5 s; ~500 signal streams got the process OOM-killed under
its production `mem_limit` of 32 MiB.

Each bound is named once, with its measurement, in
`src/http/middleware/capacity.rs`:

| Bound | Value | Why |
|---|---|---|
| `STATEMENT_TIMEOUT` | 8 s | Postgres cancels a statement itself (`57014`): cutting the request client-side leaves its statement running |
| `REQUEST_TIMEOUT` | 10 s | A request that has not produced its response is answered `503`. It bounds the response's production, not its body, so the signal stream is not cut |
| `WORK_SLOTS` | 6 | All expensive reads share 6 `WorkSlots` on a 10-connection pool: the slow routes — `/api/pools`, `/api/pools/{address}`, `/api/pools/{address}/history`, `/api/signals` — and the cached computations below. With the signal poller's connection, the light routes always keep 3 |
| `WORK_SLOT_WAIT` | 2 s | How long expensive work waits for a slot before `503` — enough for a page's own burst |
| `SSE_MAX_STREAMS` | 200 | ~62 KiB an open stream; half of `MAX_CONNECTIONS`, so streams never take the connections ordinary requests need |
| `MAX_CONNECTIONS` | 400 | Connections held at once; the next wait in the kernel's accept queue (`CappedListener`). Capping streams was not enough: a burst of connections costs memory even when refused, and the allocator keeps the peak — 1000 at once got the process OOM-killed under 32 MiB with the stream cap in place. The `yog-api` image sets `MALLOC_ARENA_MAX=2`: without it glibc kept each burst's peak in a new arena (29.7 → 49 MiB → OOM over three rounds of 1000 opens); with it, twelve rounds level off at 44.1 MiB, inside the production `mem_limit`, raised to 64 MiB for it |
| `IDLE_TIMEOUT` | 30 s | A connection with no byte either way for this long is closed, and its slot comes back. Without it the connection cap was a cheaper outage than the one it prevents: axum's `serve` sets no header-read nor keep-alive timeout, so 400 silent sockets would hold every slot. Both directions count — an SSE client sends nothing, its 15 s ping keeps it alive. It bounds that outage (400 silent sockets: `/healthz` answered after 28 s), it does not remove it for a client that can reach the process directly — in production only Caddy can, and it opens an upstream connection only for a complete request |

And two results are shared rather than bounded: **`/api/pools/top` and
`/api/stats`** return the same body to every visitor, so they go through
`SharedCache` (`application/cache.rs`, on `moka`): the first caller of a cold
key computes, the others wait for the same outcome — an error included, and
nothing failed is kept. Each use declares a named `CachePolicy` (time to
live, capacity) in that file: `TOP_POOLS` (30 s, 2 entries, one per `PoolRankMetric` —
the ranking is computed at 20 and cut to each `limit`) and `STATS` (30 s,
1 entry). The capacity, with eviction, is what makes the cache safe on keys
that are not a fixed set.

To cache a new result: add its policy to `cache.rs` with the reason for each
number, hold a `SharedCache` in the service, and take a work slot **inside**
the computation, never around the call.

Their **routes** stay outside the `work_slot_limit` middleware; their
**computations** take a work slot, inside the cache. So the caller that
computes holds a slot and the callers waiting for its value hold none.
Measured with the routes behind the middleware: 30 parallel `/top` on a cold
cache gave 24 × `503` for a single computation, each waiter holding a slot.

What this does not do: limit a client by IP or key. That is the edge's job in
production, and the platform API's later.

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
| 400 | `Bad Request` | Invalid address, malformed cursor, limit out of range, mutually exclusive params |
| 404 | `Not Found` | Pool or token unknown, no observed state yet for a known pool |
| 503 | `Service Unavailable` | Over capacity: no work slot within 2 s, too many open signal streams, the request past its deadline, or the database out of connections or past `statement_timeout`. Carries `Retry-After`; see [Capacity bounds](#capacity-bounds) |
| 500 | `Internal Server Error` | DB failure, encoding bug. `detail` is always the generic message; the real cause is logged server-side under a `request_id` correlatable via the `x-request-id` response header |

⚠️ A query string axum cannot parse — an unknown `severity` or `metric`, a
non-numeric `limit` — is refused **400 in plain text** by the extractor, before
any `ApiError` exists.

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
