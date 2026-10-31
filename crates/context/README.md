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
├── application/     ← what the daemon does once wired
│   ├── source/      ← ports: MetadataSource, PriceSource, PoolAccountSource
│   └── workers/     ← use cases: MetadataWorker, PriceWorker, PoolAccountWorker,
│                      one <worker>.rs each, its own modules in <worker>/ —
│                      metrics, log lines (log.rs, one function per line) and
│                      tests, and a tick_outcome.rs (price/, metadata/):
│                      TickOutcome, the ways a cycle ends — returning one is
│                      what makes a silent exit fail to compile
├── infra/           ← adapters: HeliusDasClient, JupiterPriceClient,
│                      SolanaAccountClient (+ provider metrics; jupiter_price/
│                      log.rs holds the client's log lines; infra.rs holds
│                      the shared http_client and its timeouts; source_error.rs
│                      turns a reqwest failure into a SourceError, URL stripped)
├── bootstrap/       ← Config::load(), Daemon::connect — composition root, and the
│                      stop: run() joins its three workers under the shared grace
├── error/           ← SourceError, WorkerError — shared by the layers
└── main.rs
```

The ports/adapters split keeps the workers testable: a worker depends on a
`source` trait, never on the concrete HTTP client. Providers chunk and fetch
internally; a worker makes a single `fetch_*` call per tick and upserts what
came back.

Providers are **transport only** — they do not interpret payloads. The account
source returns raw bytes plus the owner; base64 is the RPC's encoding, so it is
decoded at this boundary and never reaches `core`, which stays free of it.

## Three workers, two cadences

- **`MetadataWorker`** — every `CONTEXT_METADATA_POLL_SECS` (default 10 s),
  lists the mints absent from `token_metadata` and fetches symbol / name /
  decimals / logo via Helius DAS. A tick reads only the pools discovered in
  the last hour (`list_missing_mints_of_recent_pools`, over
  `MISSING_MINT_DISCOVERY_WINDOW` and `idx_pools_first_seen_at`); the first
  tick, then one an hour, sweeps every pool (`list_missing_mints_of_all_pools`).

  ⚠️ **Only the sweep sees the mints filled in late.** The `PoolAccountWorker`
  writes the mints of pools seen without them, whose `first_seen_at` is old;
  so do mints Helius left unresolved past the hour. Either waits up to an hour.
  A sweep counts as done once listed: only a failed listing sweeps again at the
  next tick, a Helius failure waits for the next sweep. Its metrics carry a
  `scope=recent_pools|all_pools` label.
- **`PriceWorker`** — every `CONTEXT_PRICE_INTERVAL_SECS` (default 30 s), or
  back to back when a tick outlasts it, lists the candidate mints and asks
  Jupiter Price V3 for current USD prices, inserting them with a single shared
  `fetched_at` per tick.

  **A candidate is a known mint still alive somewhere.** One of its pools had
  an event in the last 24 hours (`pool_current_state.last_event_at`), or
  Jupiter priced it in the 24 hours before the last price written
  (`token_last_price.fetched_at`): `TokenMetadataRepository::list_price_candidates`,
  over `PRICE_CANDIDATE_WINDOW` in `yog-core`. A mint quiet on both counts is
  not asked again until an event on one of its pools brings it back.

  ⚠️ **The price window counts back from the last write, not from now.** After
  an outage longer than the window, every price is older than 24 hours, and a
  window counted from now would drop the mints of every quiet pool for good.

  ⚠️ **Asking the whole of `token_metadata` empties the TVL.** Most known mints
  never get a price, and when they all fall due at once the tick outlasts the
  15-minute bound on prices (`yog_price_max_age_latest()`), during which every
  pool loses its valuation.

  **A candidate is asked at a cadence set by its class** (`PriceCandidate` in
  `yog-core`), because the length of a tick follows the mints it asks:

  | Class | Which mints, in this order | Asked |
  |---|---|---|
  | `NeverPriced` | no price ever, and known for over an hour | one tick in 105 |
  | `LowValue` | what its pools hold, at its last price, is under 10 $ | one tick in 3 |
  | `Regular` | everything else | every tick |

  `list_price_candidates` reads the class from the database at every tick. A
  mint's turn is drawn from its address and the tick number, so a class spreads
  evenly over its ticks and a restart does not bring it all back at once. The
  tick number restarts from 0, so a restart starts a mint's wait over: up to a
  whole cycle of its class, and a request that fails at its turn costs the same.
  The rule below applies on top of it. `yog_context_price_candidates_by_class`
  gives each class's size.

  ⚠️ **The length of a tick is what keeps a motionless price valid**: it is
  rewritten at the first turn past the floor below, within 15 minutes for a
  `Regular` mint up to ticks of 7 minutes, and for a `LowValue` mint between
  ticks of 2 min 40 and 5 minutes. A `CONTEXT_PRICE_INTERVAL_SECS` of 300 s or
  more puts `LowValue` mints out of that range.

  **It asks only for the candidates worth asking.** A mint Jupiter answers
  **without a price** waits before it is asked again — 1, 2, 4, 8, then 15
  minutes at the default cadence — and waits no more at its first
  price. The rule is `UnpricedMints` in `yog-core`, in memory and empty at
  boot; a mint that leaves the candidates leaves it too. Where half the
  universe never has a price, it halves the requests.

  ⚠️ **Only an answer without a price counts.** A chunk given up on 429 was
  never answered, and neither was a chunk without a single price (`{}`, an
  error body, all-null entries — logged as degraded): their mints keep their
  schedule. The rule is fed before the worker's filters, so a price the column
  cannot store still counts.

  **It drops prices the price column cannot hold**, before the insert, counting
  them in `yog_context_price_rejected_total` and naming the mints in a `warn!`.
  The test is `TokenPrice::is_storable`, and it bounds `NUMERIC(38, 18)` at both
  ends — never `> 0`:

  - under `5e-19` a price rounds to exactly `0` on write (positive right up until
    the coercion), and a stored zero silently passes every `price_usd IS NULL`
    guard downstream — `23514` from the `CHECK` of migration 009;
  - at or above `1e20` it exceeds the type's 20 integer digits — `22003`, raised
    by the column *type* while coercing, so 009's constraint never even runs.
    `usd_price` arrives from Jupiter unvalidated, so nothing upstream bounds it.

  The filter is not a nicety: `insert_batch` sends the whole tick as one
  statement, and `ON CONFLICT DO NOTHING` covers neither SQLSTATE, so **either**
  failure aborts the insert for every other mint — every tick, for as long as
  that mint stays a candidate, leaving an as-of gap that migration 005 established
  never heals. The database is the guarantee; this filter is what keeps it from
  ever firing.

  **It also writes only what says something new.** A price identical to the
  last row kept for its mint earns no row of its own. Without that the series
  grows at the rate of the worker rather than at the rate of the prices — 70 to
  82 % of the rows repeated their predecessor (measured 22 September 2026). The
  rule is
  `KeptPrices` in `yog-core` — a product judgement about freshness, not a
  storage trick — and it turns on two things that are easy to get wrong:

  - **a floor.** A motionless price is written anyway once the last kept row
    reaches 8 minutes 30, chosen under the 15 minutes of
    `yog_price_max_age_latest()` (migration 005) by more than a tick's length,
    and under three ticks so that a `LowValue` mint is rewritten at each of its
    turns. Without it a stable token's
    last observation ages out of both staleness windows and its USD figures
    turn NULL — a worse defect than the volume it saves.
  - **rounding to the column's scale.** `NUMERIC(38, 18)` rounds on write, and
    80 % of the rows measured carry exactly 18 decimals, so the value Jupiter
    sent is not the value the table holds. The comparison therefore rounds
    first; comparing the raw `Decimal`s would find almost no two prices equal
    and suppress nothing at all.

  The worker holds the last kept row per mint **in memory** — it is the only
  writer of `token_prices`, so nothing else could tell it — and records a row
  only after the insert succeeds. A restart forgets everything, which costs one
  full batch at the first tick: a row too many, never one too few.

  The floor is set **one tick early** (`KeptPrices::new` takes the cadence), so
  the forced row lands at 8 minutes 30 and not at the first tick past them. At a
  cadence where ticks hold to it, that spaces rows evenly; a tick that outlasts
  the cadence still lands the row late, which is why its length is bounded above.

  ⚠️ **Above 255 s of cadence the rule saves nothing.** Two kept rows are at
  most 8 minutes 30 apart, so a cadence over half of that leaves no room for a
  suppressed tick and every tick writes. Correct — freshness beats volume — but
  it means raising the cadence past that silently costs the whole
  benefit, with `yog_context_price_unchanged_total` flat at 0. The startup line
  says which regime you are in: `rewrite_after_secs` equal to the
  cadence means nothing is being suppressed. See `CONTEXT_PRICE_INTERVAL_SECS`
  under *Configuration* for the two cadences that are refused outright, and why
  they are not this filter's doing.

  ⚠️ Ideally this worker is already running when 009 is applied, but
  `docker-compose.yml` orders `yog-context` *after* `yog-migrate`, so the plain
  `up --build` cannot do it — see the deployment note in
  `009_price_positivity.sql` for the manual sequence and for why the plain path
  is nevertheless fine until the rejection counter leaves zero.
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

One refinement on the Jupiter side: chunks are **spaced**, one every
`request_spacing` — 1.1 s for `JUPITER_RATE_LIMIT_PER_MINUTE=60`, ten requests
where the limit allows eleven, counted from the start of one request to the
start of the next. A
chunk sent back to back with the others is what drew the 429s: Jupiter counts
over a sliding minute and lets a burst through before it refuses the rest. A
chunk refused anyway is retried a bounded number of times (`Retry-After` when
present, capped exponential backoff otherwise, and never before its slot: a
retry is a request too) before falling back to skip-and-log.

⚠️ The spacing is what bounds a tick: about `request_spacing` times the chunks
asked. Past the cadence, ticks run back to back — at 60 per minute, 27 chunks
(about 1 350 mints) fit a 30 s cadence, and a price's staleness bound allows
about 7 minutes of tick, or some 19 000 mints asked.

### One invariant, and it is not optional

**Never build `SourceError::Http` or `SourceError::Decode` from a
`reqwest::Error` by hand.** Use `?`, which goes through
`impl From<reqwest::Error> for SourceError` in `infra/source_error.rs` —
with the clients, since it is transport knowledge; `error/source.rs` keeps
only the enum the ports return.

That conversion calls `reqwest::Error::without_url()` before formatting, and
that is the only thing standing between an API key and the logs: reqwest puts
the **full URL** in its own `Display`, query string included, and its
documentation says so. On 2 September 2026 nine call sites each recopied
`e.to_string()` unredacted, and one 30-minute provider outage wrote the Helius
key into **38 log lines**, all produced by that single outage: the leak rate
follows the *provider's* error rate, not ours.

The redaction sits at the conversion, not at the `warn!`/`error!` call sites,
so that a site nobody thought of cannot leak. (`yog-indexer` used to make the
other choice — `utils::redact_api_key`, applied when formatting. It stayed
`pub(crate)` and never reached this crate, which is how the leak happened; it
was removed in September 2026, being blind to a credential in a URL path, and
the indexer now scrubs at its own boundaries through `SecretUrl::scrub`.) The
conversion also appends the error's cause chain, because
stripping the URL from reqwest's `Display` otherwise leaves the same four
words for a refused connection, a DNS failure and a timeout alike.

### The stop joins its three workers

`main` owns the `CancellationToken` and cancels it on **SIGINT or SIGTERM**
(`yog_bootstrap::shutdown_signal`); `Daemon::run` then waits for the three
workers under `yog_bootstrap::SHUTDOWN_GRACE`, spent across them by the shared
`Stop`. A worker's tick body runs *outside* its `select!`, so a tick already
started always finishes — if it is given the time.

The **price worker is waited on first**, because it is the only one whose
interrupted work is *lost* rather than merely *abandoned*: its tick ends in a
single `INSERT` of prices stamped at one instant, and that instant does not come
back. The other two re-list their remainder next tick.

⚠️ **A price tick outlasts the grace on its own, so it hears the stop at any
point.** The Jupiter client races the token against each chunk — its slot,
its request and its retries — so it drops the request in flight, sends nothing
more, and returns what was answered before; the worker writes that through its
usual filters and single `INSERT`. At most one chunk, 50 mints, is lost. Two
log lines say so: the client's (`finished`, `abandoned`) and the worker's
(`outcome`, `written`), which tells a tick written from one abandoned.

⚠️ Until 14 September 2026 the daemon selected on `ctrl_c()` and returned on it:
20 stops measured, **none** saw the three workers hand back, and the process was
gone in 5–7 ms. Under `docker compose stop` it was worse still — SIGTERM was not
listened for, and as PID 1 the process does not die on it either, so the stop
never started and Docker's SIGKILL arrived ten seconds later mid-tick.

There is deliberately no in-process respawn logic: a worker never returns
`Err` from its loop, and a panic exits the whole process, which the container
restart policy relaunches with a fresh budget. The failure mode that policy
cannot see — a provider call hanging forever with the process still alive —
is closed by the shared `infra::http_client()`: every provider client
carries a total-request timeout (15 s) and a connect timeout (5 s), so a hang
degrades into a tick-level `SourceError` absorbed like any other.

## Observability

Prometheus metrics on `:9000/metrics` (host port `9001` in compose): per-worker
tick/upsert/failure counters and per-provider request counters and durations.

**Price coverage** is the one gauge pair worth alerting on:

```promql
yog_context_price_priced_mints / yog_context_price_requested_mints
```

Everything USD-denominated downstream — pool TVL, 24h volume, realized fees,
half the signal detectors — is only as complete as this ratio, and it degrades
silently: an unlisted mint or a run of Jupiter 429s simply produces unvaluable
hours. The API reports the consequence per pool (`swapBucketsPriced24h` /
`swapBuckets24h`); this is the cause, and the leading indicator.

A tick that reached Jupiter but priced nothing sets the gauge to 0 and records
`yog_context_price_tick_total{outcome="no_prices"}` — it must not look like a
tick that never ran.

**Requested mints** say how much of the candidates is due this tick:

```promql
yog_context_price_requested_mints / yog_context_price_candidate_mints
```

The gap is the mints out of their class's turn, and those waiting after an
answer without a price, for at most the
`unpriced_wait_cap_secs` of the startup line plus one cycle. A
mint that regains its price while it waits counts as unpriced until its turn. A
tick where **every** candidate waits asks nothing and records
`outcome="nothing_due"`, never `no_prices`.

A tick that priced everything and wrote nothing because nothing moved records
`outcome="unchanged"` instead, and **that one is the normal case** — most ticks
land there. The two must never share a label: `no_prices` is the anomaly to
alert on, and merging them would leave the alert permanently lit.

⚠️ **The numerator sits between the worker's two filters, and each side is a
decision.** It counts the prices **kept**, not the ones Jupiter returned: a
price refused as unstorable is, downstream, exactly as absent as one the source
never sent, so counting it would inflate the coverage the gauge exists to
measure. `outcome="no_prices"` therefore also covers a tick whose every price was
refused; `yog_context_price_rejected_total` tells the two apart and should sit
flat at 0 — a rising count means a very-high-supply mint entered the candidates,
and that its USD figures will be **absent rather than wrong**. But it counts
them *before* the redundancy filter, for the opposite reason: a price suppressed
as unchanged is still valued downstream, by the row that already carries it.
Counting after would read as a coverage collapse to under a fifth, on a system
losing nothing.

**Redundancy of the series** is the second ratio, and the one this worker is
judged on:

```promql
sum(rate(yog_context_price_unchanged_total[1h]))
  / sum(rate(yog_context_price_unchanged_total[1h])
        + rate(yog_context_price_inserted_total[1h]))
```

It answers "how much of what we fetch is worth storing". It should sit high —
four rows in five suppressed is the design, not a fault. A collapse towards 0
means the market genuinely moved, or a tick outlasted half the floor, or the
comparison stopped rounding to the price column's scale — the last making the
filter inert without failing anything.

⚠️ **`rate()` on both sides, not the bare counters.** A ratio of lifetime
counters is a process-lifetime average: it moves far too slowly to show the
collapse this expression exists to detect, and the cold-start batch — every
candidate priced on the first tick after a restart — biases it downwards for
good.

## Configuration

```env
DATABASE_URL_CONTEXT=postgresql://yog_context:...@localhost:5433/yog_sothoth
TOKEN_METADATA_URL=https://mainnet.helius-rpc.com/?api-key={key}
TOKEN_METADATA_KEY=...
POOL_ACCOUNT_URL=https://api.mainnet-beta.solana.com
JUPITER_URL=https://api.jup.ag
JUPITER_API_KEY=...
JUPITER_RATE_LIMIT_PER_MINUTE=60   # optional, the key's tier as Jupiter documents it
CONTEXT_METADATA_POLL_SECS=10
CONTEXT_PRICE_INTERVAL_SECS=30
```

⚠️ **`CONTEXT_PRICE_INTERVAL_SECS` must be non-zero and under 900 s, and the
daemon refuses to start otherwise.** Zero panics the ticker inside the spawned
worker, long after startup reported success; 900 s or more leaves the newest
price older than `yog_price_max_age_latest()` before the next tick even fires,
whether or not anything is being suppressed. Neither refusal comes from the
redundancy filter: `KeptPrices` sets its floor one tick early so that it adds
nothing to what the cycle already costs. The cadence bounds when a tick starts,
not the age of the newest price: a tick's own length — about `request_spacing`
per chunk — adds to it.

**Two Solana endpoints, and this crate is why they are two.** The DAS
(`getAssetBatch`) is Helius' own API; `getMultipleAccounts` is standard Solana
JSON-RPC that any provider serves. Both used to read one `SOLANA_RPC_HTTP` —
a name describing a transport, which excluded neither — so a migration moving
one and not the other could not be *expressed*: one variable, one value. They
are now two `Endpoint`s, each a `<FUNCTION>_URL` / `<FUNCTION>_KEY` pair; a URL
with no `{key}` and no key is a public endpoint. `POOL_ACCOUNT_URL` above shows
that shape, and in local development all five endpoints of the workspace point
at the same public host.

Nothing carrying a secret reaches the daemon as a `String`.
`DATABASE_URL_CONTEXT` is a `SecretUrl` — userinfo, path, query string and
fragment redacted, scheme, host and port legible, and the Postgres URL also
keeps its role and database name; so is each endpoint once assembled. A
`reqwest` error never carries it back: `infra/source_error.rs` strips the URL
(`without_url`) before anything reads the error.
`JUPITER_API_KEY` is a `SecretKey`, masked whole: a bare key has no carrier
worth showing, and it is what `SecretUrl` used to return unredacted for want
of a `?`. `JUPITER_URL` is a plain `String` on purpose — Jupiter authenticates
by header, so that URL hides nothing, and that is also why it is **not** an
`Endpoint`: no key ever enters it.

The type reaches the wire: `HeliusDasClient` and `SolanaAccountClient` hold a
`SecretUrl` until `.post(…)`, `JupiterPriceClient` holds a `SecretKey` until
the `x-api-key` header is built. See [`yog-bootstrap`](../bootstrap/README.md#secrets--one-invariant-two-types)
for the invariant and the guard that enforces it.

Connects to Postgres as `yog_context` — RW on `token_metadata` and
`token_last_price`, INSERT-only on `token_prices`, `UPDATE` on the
pool-property columns of `pools`, RO otherwise.

## Run

```bash
cargo run -p yog-context
```
