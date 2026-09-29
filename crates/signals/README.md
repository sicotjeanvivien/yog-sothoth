# yog-signals

Native binary. The signal engine — runs pattern detectors over the data
accumulated by the indexer and the enrichment daemon, and emits typed alerts
into the `signals` table. The API is the only egress: it serves the feed
(`GET /api/signals`) and the live SSE stream; this process never pushes
anywhere itself.

For the workspace-level picture (dependency graph, conventions, database
roles), see [`crates/README.md`](../README.md). The `SignalDetector` contract
and the `Signal`/`Severity` domain types live in
[`yog-core`](../core/README.md) so detectors depend on traits only.

---

## Layout

```
signals/src/
├── engine/        ← SignalEngine: one poll loop per detector, dedup, persist
│   └── metrics.rs ← tick and emitted counters
├── detectors/     ← one module per detector
│   ├── flow_imbalance.rs
│   ├── price_oracle_deviation.rs
│   ├── tvl_drain.rs
│   └── metrics.rs ← considered and skipped counters, SkipReason
├── materialization_alarm/ ← the continuous aggregates' alarm (see below)
│   ├── alarm.rs   ← the loop and one check: read, measure, judge, signal
│   ├── verdict.rs ← Verdict / Failure, and what a failure says
│   └── metrics.rs ← pending gauge, checks counter, undelivered pings
├── bootstrap/     ← Config::load() (config/types/: the alarm's own settings),
│                    Daemon (daemon/init.rs: the alarm's wiring)
└── main.rs

Each metric lives beside what it measures; the names on `/metrics` are all
`yog_signals_*`.
```

## Evaluation model — batch, per-detector cadence, stateless

Detectors are **batch evaluators**, not stream processors. Each
`SignalDetector` declares its own `interval()` and recomputes from a DB
snapshot at every tick — stateless between ticks, the database carries the
state. The engine runs one poll loop per detector (a second detector is a
second loop in the `JoinSet`; the engine itself doesn't change), applies
skip-and-log per tick, and shuts down via the shared `CancellationToken`.

Injection follows the same model as the indexer's persistors: **the detector
owns its repositories**, injected as concrete `Pg*` instances by the binary at
construction, typed as `core` traits. The `EvalContext` is thin — it carries
only the tick clock (`evaluated_at`), so windows are computed from a fixed
point and `triggered_at` is coherent across a tick. Detectors *return*
`Vec<Signal>`; the **engine** owns the `SignalRepository` and persists.

## Deduplication — cooldown with escalation override

A batch detector re-emits the same conclusion every tick as long as the
condition holds. The engine deduplicates: each detector declares a
`cooldown()`, and a candidate whose `(detector, pool)` was already signalled
within that window is dropped — **unless** its severity is higher than the
previous signal's, which overrides the suppression. The lookup
(`SignalRepository::latest_severity_by_pool`) is a query, not a DB unique
index: TimescaleDB requires the partition key (`triggered_at`) in unique
indexes, which differs at every tick. Stateless like everything else — the DB
carries the dedup state too.

## Stopping

`Daemon::run` owns the `CancellationToken` and cancels it on **SIGINT or
SIGTERM** (`yog_bootstrap::shutdown_signal`); every detector loop then returns
at its next turn, and `SignalEngine::run` joins them. A tick runs in the body
of its `select!` arm, not inside the `select!`, so a tick already started
always finishes. The materialisation alarm is the exception, on purpose: its
check — a read, then a ping with a 15 s timeout — races the token, since a ping
cut short costs nothing and a stop held by a slow endpoint is what Docker ends
with SIGKILL.

- **The stop wins a tie.** The loop's `select!` is `biased`, with the
  cancellation arm first. The ticker keeps tokio's default
  `MissedTickBehavior::Burst`, so a tick that outran its interval leaves the
  next one already ready: unbiased, about one stop in two would start a fresh
  round of reads and inserts *after* the stop was asked for.
- **A cancelled task is not a panic.** A `JoinError` goes through
  `yog_bootstrap::TaskEnd`: only a panic cancels the other detectors and ends
  the engine in `EngineError::DetectorPanicked`.

⚠️ Until 14 September 2026 the daemon waited on `ctrl_c()` alone. `docker
compose stop` sends SIGTERM, and as PID 1 the process does not die on it
either, so the stop never started and Docker's SIGKILL arrived ten seconds
later, mid-tick.

⚠️ **The other half is still missing.** Unlike `yog-indexer` and
`yog-context`, the engine joins its detectors **with no deadline** — no
`yog_bootstrap::Stop`, no `SHUTDOWN_GRACE`. A detector stuck in a slow query
holds the stop open until Docker's SIGKILL, and nothing in the logs names it.
Not fixed yet.

## The materialisation alarm

The detectors read hourly continuous aggregates that TimescaleDB's scheduler
keeps materialised. When it stops — or one refresh policy keeps failing —
nothing errors anywhere: the aggregates freeze, and every detector goes on
evaluating hours that no longer change. From 16 June to 10 August 2026 all four
sat that way unnoticed. `materialization_alarm/` is the alarm for it: a loop
beside the engine, on the same stop.

Every `SIGNALS_MATERIALIZATION_INTERVAL_SECS` it calls
`yog_cagg_materialization_backlog()` (migration 013), which reports, for every
aggregate the catalog holds, the **oldest raw row not yet materialised**. An
aggregate is late when that row has waited longer than
`SIGNALS_MATERIALIZATION_MAX_WAIT_MINS` — `MaterializationBacklog::late_by`
in `yog-core`.

⚠️ **The oldest pending row, and neither the clock nor the newest row.** The
watermark's age against the clock grows when the *indexer* stops (no bucket
fills), and would blame the materialisation. The newest row minus the
watermark freezes when a table stops receiving rows, however long those rows
then wait — on 29 September 2026 it read 28 minutes for six `claim_reward` rows
unmaterialised for eight days. A pending row only grows old if a refresh did not
run.

⚠️ **One exception: rows written late.** The wait runs from a row's block time,
not from its insertion. When the indexer catches up after an outage longer than
the limit, the rows it writes arrive already "old", and the check fails until
the next hourly refresh materialises them — at most an hour. The outage itself
is the indexer's dead man's switch to report; this one follows it briefly.

**The limit.** A healthy aggregate peaks at three hours: the refresh policy's
`end_offset` (1 h), up to an hour until the next hourly run, and the bucket
itself (1 h) — migration 008. The default of four leaves an hour of margin.
The peak observed in production is the number that should replace this
estimate.

**The alarm is Healthchecks.io, not Prometheus** — nothing scrapes `/metrics`
in production. Each check ends in a ping to
`SIGNALS_MATERIALIZATION_HEARTBEAT_URL`: success when nothing is late,
`/fail` naming each late aggregate and its wait, or `/fail` with the database's
error when the backlogs cannot be read — or does not answer within a minute
(the pool sets no `statement_timeout`). A stopped daemon is the silence the
check notices on its own. The heartbeat is `yog_bootstrap`'s, shared with
`yog-archive`. Create the check with a **10-minute period and a 20-minute
grace**.

Without that URL — development, where the scheduler is off by design and every
aggregate is late for good — nothing is signalled; the verdict is logged **when
it changes**, not on every check, and measured.
`docker-compose.prod.yml` refuses to start without it.

## Detectors

**`flow_imbalance`** — directional swap-flow imbalance over a rolling window:
`(a_to_b − b_to_a) / (a_to_b + b_to_a)` on USD-valued volumes read from the
`meteora_damm_v2_pool_hourly_flow` VIEW (baseline §15). A volume floor
filters thin pools. Warning at `|imbalance| ≥ threshold`, Critical at
`≥ critical`.

**`price_oracle_deviation`** — compares the on-chain spot price (decoded from
`sqrt_price` Q64.64 via `core::amm`) with the oracle price
(`price_a_usd / price_b_usd` from Jupiter), on the relative gap
`(spot − oracle) / oracle`, reading the `pool_price_snapshot` VIEW (migration
024). **Freshness guards on both sides**: a stale oracle price or a pool whose
last swap is too old makes the comparison meaningless — no signal is emitted
rather than a false one. The Warning/Critical scale is validated fail-loud at
config load (`threshold < critical`, otherwise Warning would be unreachable).

**`tvl_drain`** — a pool being emptied of its liquidity (LP exodus, rug-like
behaviour): over a rolling window, `drain = net_removed / starting TVL`, where
`net_removed = removed_usd − added_usd` (LP churn nets out) and the starting
TVL is the current TVL plus what left. Reads the
`meteora_damm_v2_pool_hourly_liquidity_flow` VIEW (baseline §15) joined with
`pool_current_tvl` (§15 too). Guards: an unvaluable TVL (unknown price, unresolved
mints, no reconstructed state) skips the pool — no signal beats a fake one;
the TVL floor applies to the *starting* TVL so a pool drained within the
window can't dodge the floor by having drained itself below it.

All detectors record in `signal.threshold` the boundary that justifies the
emitted severity: the critical threshold on a Critical signal, the warning
one otherwise — not the emission floor.

## Configuration

```env
DATABASE_URL_SIGNALS=postgresql://yog_signals:...@localhost:5433/yog_sothoth

# flow_imbalance
SIGNALS_FLOW_INTERVAL_SECS=300        # tick cadence
SIGNALS_FLOW_WINDOW_HOURS=24          # rolling window
SIGNALS_FLOW_MIN_VOLUME_USD=10000     # volume floor
SIGNALS_FLOW_THRESHOLD=0.6            # Warning
SIGNALS_FLOW_CRITICAL=0.9             # Critical
SIGNALS_FLOW_COOLDOWN_HOURS=6

# price_oracle_deviation
SIGNALS_PRICE_DEVIATION_INTERVAL_SECS=300
SIGNALS_PRICE_DEVIATION_THRESHOLD=0.05
SIGNALS_PRICE_DEVIATION_CRITICAL=0.2
SIGNALS_PRICE_DEVIATION_COOLDOWN_HOURS=6
SIGNALS_PRICE_DEVIATION_MAX_PRICE_AGE_MINS=15   # oracle freshness guard
SIGNALS_PRICE_DEVIATION_MAX_SPOT_AGE_HOURS=24   # last-swap freshness guard

# tvl_drain
SIGNALS_TVL_DRAIN_INTERVAL_SECS=300
SIGNALS_TVL_DRAIN_WINDOW_HOURS=6      # short on purpose: a drain is fast
SIGNALS_TVL_DRAIN_MIN_TVL_USD=10000   # floor on the STARTING TVL
SIGNALS_TVL_DRAIN_THRESHOLD=0.5       # Warning
SIGNALS_TVL_DRAIN_CRITICAL=0.8        # Critical
SIGNALS_TVL_DRAIN_COOLDOWN_HOURS=6

# materialisation alarm
SIGNALS_MATERIALIZATION_INTERVAL_SECS=600       # > 0, refused at startup otherwise
SIGNALS_MATERIALIZATION_MAX_WAIT_MINS=240       # how long a raw row may wait
SIGNALS_MATERIALIZATION_HEARTBEAT_URL=https://hc-ping.com/...  # optional; required in prod
```

Connects to Postgres as `yog_signals` — `INSERT` (append-only) on `signals`,
`SELECT` on the read VIEWs it evaluates, and `EXECUTE` on
`yog_cagg_materialization_backlog()`, which no other role holds. It cannot
update or delete anything.

## Observability

Prometheus metrics on `:9000/metrics` (host port `9002` in compose):
per-detector tick counters, evaluation durations, emitted/suppressed signal
counts, failure counters.

**`yog_signals_skipped_total{detector, reason}`** counts pools a detector
declined to evaluate — `unpriced` (the window was not entirely valuable),
`no_tvl` (current TVL unpriceable), `stale` (an input older than its freshness
gate), `no_decoder` (no `sqrt_price` decoder shipped for that protocol —
missing code, not a data problem), `undecodable` (an oracle ratio that will not
compute). The labels are defined once, by the `SkipReason` enum in
`metrics.rs`, and the counter's `# HELP` text is built from it — this list
copies it for the reader, not for the code. Emitting nothing is the right answer
to a pool we cannot value; staying *quiet* about how often that happens is not,
because degrading price coverage would then look exactly like a calm market.

⚠️ Alert on **`skipped / considered`**, using
`yog_signals_considered_total{detector}` — the pools a tick was handed, before
any guard. Not `skipped / tick_total`: `skipped` counts per POOL and `tick_total`
per TICK, so that ratio reads "pools skipped per run" and moves with the pool
count rather than with coverage. It would sit near 32 forever and tell you
nothing.

⚠️ `considered` counts every pool handed to the detector, including those that
then fall below `min_volume_usd` / `min_tvl_usd`. Those are *evaluated and judged
immaterial*, not unseen, and they are deliberately NOT counted as skips — so
`skipped / considered` is the share we cannot see out of everything we looked at,
not out of everything we could have signalled on.

Measured on the dev database on 7 August 2026, over a 24 h window: 68 pools of
100 evaluated, 32 skipped — **all 32 because neither of the pool's two tokens has
any price at all**, so the implied rate of migration 002 has nothing to anchor
on. None came from unresolved metadata. A skip is a pool nothing could have
valued, not one that was given up on; the number moves when `yog-context` prices
more mints, and nowhere else.

**`yog_signals_materialization_pending_seconds{aggregate}`** — how long each
aggregate's oldest unmaterialised row has waited, `0` when none waits. ⚠️ A
check that cannot read the progress has no reading to give, so the gauge keeps
its last one: read it next to `…_checks_total{outcome="unreadable"}`;
**`yog_signals_materialization_checks_total{outcome}`** — `on_time`, `late`,
`unreadable`; **`yog_signals_heartbeat_failures_total{kind}`** — pings that
could not be delivered.

## Run

```bash
cargo run -p yog-signals
```

## Adding a detector

1. Implement `SignalDetector` (from `yog-core`) in a new module under
   `detectors/`, owning the repository traits it reads. If the read shape
   doesn't exist yet, add a read model + VIEW following the
   `swap_flow`/`pool_price_snapshot` pattern (VIEW in a migration, `GRANT
   SELECT … TO yog_signals`, slim repo in `persistence`).
2. Wire it in `bootstrap/daemon.rs` with its concrete `Pg*` repos and its
   config block — a new loop joins the `JoinSet`; the engine is untouched.
3. Unit-test the decision function against synthetic snapshots (see
   `*_tests.rs` next to each detector).
