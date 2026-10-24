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
│   ├── services/          ← cross-protocol services at the root (SignalService,
│   │                        TokenService, NetworkService,
│   │                        IngestionService, AnnouncementService)
│   ├── signal_stream.rs   ← SignalStreamPoller (enriches once, feeds the SSE broadcast)
│   ├── cache.rs           ← SharedCache (moka) and its named CachePolicy per use
│   ├── enriched_token.rs  ← one token side of a pool, identity only
│   └── enriched_signal.rs ← signal + embedded token pair of its pool
├── http/
│   ├── handlers/          ← one module per route family
│   ├── dto/request/       ← query/path DTOs, validated before any DB call
│   ├── dto/response/      ← wire shapes, decoupled from the domain
│   ├── dto/schema.rs      ← the wire forms the contract names (UnsignedDecimal, Count…)
│   ├── tests/contract.rs  ← test-only: the OpenAPI document, as committed in openapi.json
│   ├── cursor.rs          ← base64/JSON cursor codec
│   ├── middleware.rs      ← CORS, security headers, request-id tracing,
│   │                        capacity bounds (middleware/capacity.rs), every
│   │                        error as Problem Details (middleware/problem_details.rs)
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

A service sits under `services/<protocol>/<product>/` only when its repository,
params and result are irreducibly that product's. Everything else stays at the
root, **including services whose only data source today is DAMM v2**.

The test is reach, not current content. A cross-protocol service must not name
a protocol in its constructor: that is how a neutral type accretes one
protocol's vocabulary, which is what migration 036 removed from the `pools`
table and what the layout above keeps out of the service layer.

The one place the read path matches on a protocol is `ProtocolResponse`
(`http/dto/response/protocol.rs`): its `From<domain::Protocol>` is exhaustive
with no wildcard arm, so adding a variant breaks the build there.

### Where a DTO goes

`http/dto/request/` and `http/dto/response/` follow one rule throughout:

- **Name.** A response type ends in `Response`. An enum takes the name of the
  domain type it translates (`domain::FreshnessStatus` →
  `FreshnessStatusResponse`), so the two never share a name — without the
  protocol prefix when its response has none.
  `every_schema_is_a_response_or_a_wire_form` fails on a schema of the
  contract named otherwise. On the request side, `*Request` is what a route
  takes once validated, `*Query` the raw query string it parses, `*Param` an
  enum of that query, named like the domain type it translates
  (`domain::Severity` → `SeverityParam`).
- **Place.** One type per file. At the top level, what a route returns — its
  body, or the item of a page — or takes, its `*Request`. A part lives in its
  owner's folder, one level deep; a response part's file drops the owner's
  prefix, its type keeps it (`announcement/kind_response.rs` holds
  `AnnouncementKindResponse`).
  A part several share lives with the one it is about, and the others reach
  it through it: `signal/severity_response.rs`,
  `token/embedded_token_response.rs`. The query of `GET /api/signals` and its
  pagination are parts of `list_signals/`, the one route that reads them.
- **Visibility.** Each item and field declares the reach it has. A field is
  private unless something outside its file builds or reads it — the handlers
  build `PageResponse` and the readiness types, `ListSignalsRequest::parse`
  reads `SignalsQuery`. `pub(super)` is for what only its folder and that
  folder's tests use, `pub(in crate::http::dto::response)` for a part other
  responses reach, `pub(crate)` for what the handlers see.
- **Conversions** live with the type they build, and a behaviour two DTOs
  share is written once, in their common parent: `logo_uri` in `token.rs`,
  `parse_address` in `request.rs`.
- **Tests** of a part go in its owner's file, `<side>/tests/<owner>_tests.rs`.

## Endpoints

| Method | Path | Description |
|---|---|---|
| `GET` | `/healthz` | Liveness — 200 OK, no DB roundtrip |
| `GET` | `/readyz` | Readiness — pings the DB; 503 with per-check detail when it fails |
| `GET` | `/api/network` | The chain's latest slot as the probe saw it, the probe's `getSlot` latency, when it was observed |
| `GET` | `/api/ingestion` | Our ingestion's freshness (`live` / `delayed` / `stale`) and its last indexed event |
| `GET` | `/api/signals` | Paginated signal feed (`triggered_at DESC, id DESC`; `?severity=` and `?pool=` filters); each item embeds its pool's token pair (`tokenA`/`tokenB`, `EmbeddedTokenResponse`) |
| `GET` | `/api/signals/stream` | SSE stream of new signals (see below) |
| `GET` | `/api/protocols` | Every protocol of the domain: observed by this build or not, pools seen — the catalogue, live |
| `GET` | `/api/protocols/activity` | Per protocol over `?window=24h\|7d\|30d` (`?offset=` for the previous one): volume, fees and their shares, swaps, distinct active pools, births, pool-hours and the valued ones, and the liquidity brought by creations, added and removed, with its moves valued |
| `GET` | `/api/protocols/activity/hourly`, `/daily` | The same flows per protocol and bar: an hour, or 24 hours ending at `asOf` |
| `GET` | `/api/protocols/{protocol}/liquidity-moves` | The protocol's valued liquidity moves (creations, additions, removals) over `?window=`, largest first, `?limit=` of them; its pool's token symbols embedded |
| `GET` | `/api/protocols/tvl` | Per protocol, the TVL of the last frozen hour and, for each window, the part its discovered pools hold |
| `GET` | `/api/protocols/tvl/hourly` | Per protocol, the TVL at the close of every hour from the window's start |
| `GET` | `/api/tokens/activity` | Tokens by volume over `?window=`, identity embedded, forward-only cursor; a swap counts for both its tokens |
| `GET` | `/api/tokens/{mint}` | Token metadata + latest price (200 with `price: null` if no price yet) |
| `GET` | `/api/announcements/active` | Operator announcements whose window is open — most severe first, non-paginated (hard cap); empty array is the nominal case |

**What yog-analytic computes** (`/api/protocols/…` but the catalogue, `/api/tokens/activity`)
read yog-analytic's tables over one span, built in `ActivityService`: the window
ends at the close of the last frozen hour, `asOf`, the same for every route, and
no read counts an open hour. Each response carries `from`, `asOf` and
`coveredSince`, the first hour of the facts: a total whose window starts before
it returns no row rather than a partial sum; a series returns the bars the facts
hold whole. The TVL is each protocol's last measure by that hour, its `at` said,
with the part of it held by the pools discovered in each window, measured with it
(a window without its part was not measured: the web then withholds the change).
The liquidity is measured from a later hour than the swaps, its own start: its
`liquidity` is `null` on a total, a bar or a ranking of moves whose window starts
before it, rather than a partial sum.
The tokens are the exception to one read of the frozen hours: their span ends at
the hour of the ranking read with them, which each freeze rewrites whole — a
cursor carries that hour, and one taken before a freeze is refused (400).

## What a route may return

These rules hold for every `/api` route. The routes that do not follow them yet are listed
at the end of this section.

1. **A route serves a resource of the data**, of one of four kinds, named in kebab-case, a
   collection in the plural:
   - **on-chain events**, what the chain emits, as indexed: `swaps`, `liquidity-changes`,
     `pool-creations`, `position-changes`, `position-fee-claims`, `protocol-fee-claims`,
     `reward-claims`, `reward-changes`, `pool-changes`;
   - **business resources**, the entities of the domain and their state, observed on the chain
     or fetched from a provider: `protocols`, `pools`, `tokens`, and a token's `prices`;
   - **produced resources**, what we compute from the first two: the measures (rule 4),
     `tvl`, `liquidity-moves`, `rotation-runs`, `vesting-unlocks`, `signals`;
   - **service resources**, the state and the messages of the service: `network` and
     `ingestion`, singletons each at `/api/<singleton>`, and `announcements`.

   A name says what is observed, never an image or a category (`activity`, `stats`,
   `metrics`, `analytics`). An on-chain collection takes the name of the on-chain event
   family it gathers (`initialize_pool` → `pool-creations`, `EvtLiquidityChange` →
   `liquidity-changes`); a produced one names what it observes: `tvl`, the value deposited, a
   reading at an instant; `liquidity-moves`, the liquidity brought by the pool creations,
   added or removed, valued; `rotation-runs`, a pool's tranches whose quoted reserve turned
   over at least once, no more than 30 min apart, with the tranches between them;
   `vesting-unlocks`, the liquidity a vesting lock releases at an instant.
2. **Three path forms**: `/api/<collection>`, every protocol; `/api/<collection>/{id}`, one
   item; `/api/<entities>/{id}/<collection>`, the collection restricted to that entity
   (`/api/protocols/meteora_damm_v2/swaps`). An entity is always in the path, never a filter
   parameter. An identifier is a key of the data — a base58 key, a protocol name, a signal's
   or an announcement's id — and never a reserved word (rules 4 and 5).
3. **A collection returns its items**, cursor-paginated (`Page<T>`: `?limit=`, 50 by
   default, 200 at most) — all but `tvl`, a reading, and `tvl/hourly`, which its window
   bounds (rule 4) — with `?window=` when it is read over one and `?sort=` when it has more
   than one order. A collection not paginated yet is listed below. A parameter may filter on
   an attribute of the items (`?severity=`), never name an entity. Every item of a collection
   shared by several protocols carries its protocol.
4. **A measure is a collection followed by a cut**, a reserved word that ends the path:
   - `totals`, the whole window: one row per protocol at the root, under an entity its own;
   - `hourly`, `daily`: the same rows, one per bar, a day being the 24 hours ending at
     `asOf`;
   - `by-token`, `by-pool`, `by-owner`: one row per token, pool or owner address, summed over
     the path's scope (every protocol at the root), a ranking paginated as a collection.

   A measure's row is not an item: it carries a protocol only where it is cut by protocol.

   `tvl` is a reading at an instant, never a sum over a period: `/tvl` gives the reading at
   `asOf`, without a window, one per protocol at the root, under a protocol its own; `hourly`, one reading per hour of the window, each at its hour's
   close, unpaginated, the window bounding them; `by-token` and `by-pool` rank at `asOf`,
   without a window. It has no `totals`. Every response of what `yog-analytic` computes carries
   the instant it reflects (`asOf`), its coverage (`coveredSince`), and its window when it is
   read over one.
5. **`live` is the reserved word of the SSE routes**: `/api/live` for every protocol,
   `/api/<entities>/{id}/live` for one entity, `?topics=` naming the collections it streams,
   one connection for them all: the on-chain events and `signals`, the collections read live
   (rule 7), never one `yog-analytic` computes. Each message keeps its own shape.
6. **Every field of a response completes the name of its route.** The route reads as a
   sentence ("the swaps of the DAMM v2 protocol over 24 h, in totals") and each field is an
   attribute of it. A field that needs another name to describe itself belongs to another
   route. A response embeds the *identity* of another resource (the symbol and logo of a
   signal's tokens), nothing else: neither its state nor its measures. A field that exists in
   one protocol only goes in a block named after it.
7. **A response does not mix what is read live and what `yog-analytic` computes.**
   - Live: what is read as it was written — the on-chain events, the business resources and
     their current state (reserves, price, a pool's current TVL), the service resources,
     fresh to the second; the signals, produced by `yog-signals`, once their detector has run.
   - Computed: everything `yog-analytic` writes — the measures, `tvl`, the collections it
     values or draws — fresh to a few minutes.
   - Mixed, the slower makes the other unavailable, and a response can no longer state the
     age of what it holds.
   - The boundary is the nature of the data, not today's cost: a measure over a window is
     computed, even if it would count fast live.
   - A computed response embeds the identity of what it measures: an identity does not age
     and costs nothing. The reverse is forbidden.
8. **A measure is read from `yog-analytic`'s tables**, never from a view that values at
   read time. That is what holds the delay whatever the window.
9. **The form rules stay as they are** — cursor pagination, Problem Details, camelCase,
   RFC 3339 — written once, in the contract.
10. **The contract is written once, on the server, and the client derives from it**: an
    OpenAPI document drawn from the Rust types, from which the web's zod is generated
    ([The contract](#the-contract)).

**Parameters and fields.** `window=24h|7d|30d`, a closed list, 400 otherwise; `offset` for
the previous window. For a collection of future events (`vesting-unlocks`), `window` covers
the period after `asOf`. A parameter never changes the shape of a response; `topics` (rule 5)
chooses which messages come, not their shape. `asOf` is the field of the instant a response
reflects. The protocol is written `meteora_damm_v2` everywhere (field, path); fields in
camelCase.

**Not yet following these rules**: `/api/protocols/activity` and its `/hourly` and `/daily`,
`/api/protocols/tvl` and its `/hourly`, `/api/tokens/activity`, `/api/announcements/active`,
`/api/signals/stream`, the `pool` filter of `/api/signals`, `/api/protocols` (no cursor),
and `/api/protocols/{protocol}/liquidity-moves` (no cursor, and a `coveredSince` that is the
swaps', not the liquidity's).

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
  `skip_serializing_if` without it); every operation declares its failures as
  Problem Details; an `int64` (or `u64`) anywhere fails the document, because the generated
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
requests to the pool ranking route, since retired, answered `500` ten times and made a 4 ms route
answer `500` after 5 s; ~500 signal streams got the process OOM-killed under
its production `mem_limit` of 32 MiB.

Each bound is named once in
`src/http/middleware/capacity.rs`:

| Bound | Value | Why |
|---|---|---|
| `STATEMENT_TIMEOUT` | 8 s | Postgres cancels a statement itself (`57014`): cutting the request client-side leaves its statement running |
| `REQUEST_TIMEOUT` | 10 s | A request that has not produced its response is answered `503`. It bounds the response's production, not its body, so the signal stream is not cut |
| `WORK_SLOTS` | 6 | All expensive reads share 6 `WorkSlots` on a 10-connection pool: the slow route — `/api/signals` — and the cached computation below. With the signal poller's connection, the light routes always keep 3 |
| `WORK_SLOT_WAIT` | 2 s | How long expensive work waits for a slot before `503` — enough for a page's own burst |
| `SSE_MAX_STREAMS` | 200 | ~62 KiB an open stream; half of `MAX_CONNECTIONS`, so streams never take the connections ordinary requests need |
| `MAX_CONNECTIONS` | 400 | Connections held at once; the next wait in the kernel's accept queue (`CappedListener`). Capping streams was not enough: a burst of connections costs memory even when refused, and the allocator keeps the peak — 1000 at once got the process OOM-killed under 32 MiB with the stream cap in place. The `yog-api` image sets `MALLOC_ARENA_MAX=2`: without it glibc kept each burst's peak in a new arena (29.7 → 49 MiB → OOM over three rounds of 1000 opens); with it, twelve rounds level off at 44.1 MiB, inside the production `mem_limit`, raised to 64 MiB for it |
| `IDLE_TIMEOUT` | 30 s | A connection with no byte either way for this long is closed, and its slot comes back. Without it the connection cap was a cheaper outage than the one it prevents: axum's `serve` sets no header-read nor keep-alive timeout, so 400 silent sockets would hold every slot. Both directions count — an SSE client sends nothing, its 15 s ping keeps it alive. It bounds that outage (400 silent sockets: `/healthz` answered after 28 s), it does not remove it for a client that can reach the process directly — in production only Caddy can, and it opens an upstream connection only for a complete request |

And one result is shared rather than bounded: **`/api/protocols`** returns
the same body to every visitor, so it goes through
`SharedCache` (`application/cache.rs`, on `moka`): the first caller of a cold
key computes, the others wait for the same outcome — an error included, and
nothing failed is kept. Each use declares a named `CachePolicy` (time to
live, capacity) in that file: `PROTOCOLS` (30 s, 1 entry) for
`/api/protocols`, which counts every pool ever seen. The capacity, with eviction, is what makes the cache safe on keys
that are not a fixed set.

To cache a new result: add its policy to `cache.rs` with the reason for each
number, hold a `SharedCache` in the service, and take a work slot **inside**
the computation, never around the call.

Its **route** stays outside the `work_slot_limit` middleware; its
**computation** takes a work slot, inside the cache. So the caller that
computes holds a slot and the callers waiting for its value hold none.
Measured with the pool ranking route behind the middleware: 30 parallel calls on a cold
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
| 400 | `Bad Request` | A query or path that does not parse, an empty value, invalid address, malformed cursor, limit out of range, mutually exclusive params |
| 404 | `Not Found` | Token unknown, no such route |
| 405 | `Method Not Allowed` | A method the route does not serve; carries `Allow` |
| 503 | `Service Unavailable` | Over capacity: no work slot within 2 s, too many open signal streams, the request past its deadline, or the database out of connections or past `statement_timeout`. Carries `Retry-After`; see [Capacity bounds](#capacity-bounds) |
| 500 | `Internal Server Error` | DB failure, encoding bug. `detail` is always the generic message; the real cause is logged server-side under a `request_id` correlatable via the `x-request-id` response header |

A parameter absent is not provided; present and empty (`?pool=`), it is an
invalid value — `400`, like any value that does not parse.

Every `/api` route answers its errors through one layer
(`middleware/problem_details.rs`): any error response that is not already
Problem Details — an extractor's refusal, a wrong method (`405`), a JSON body —
is rewritten with its status and headers kept, and a `5xx` keeps only its
canonical reason as `detail`, its text logged in the request's span. An unknown
route is the router's fallback, an `ApiError::NotFound`. Nothing to do per
route. The probes stand outside the layer: `/readyz` answers its own report.

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
curl http://127.0.0.1:5000/api/signals | jq
```
