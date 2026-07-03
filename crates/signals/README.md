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
├── engine.rs      ← SignalEngine: one poll loop per detector, dedup, persist
├── detectors/     ← one module per detector
│   ├── flow_imbalance.rs
│   └── price_oracle_deviation.rs
├── bootstrap/     ← Config::load(), Daemon (wires the Pg repos into detectors)
├── metrics.rs     ← Prometheus counters/histograms
└── main.rs
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

## Detectors

**`flow_imbalance`** — directional swap-flow imbalance over a rolling window:
`(a_to_b − b_to_a) / (a_to_b + b_to_a)` on USD-valued volumes read from the
`meteora_damm_v2_pool_hourly_flow` VIEW (migration 023). A volume floor
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
```

Connects to Postgres as `yog_signals` — `INSERT` (append-only) on `signals`,
`SELECT` on the read VIEWs it evaluates. It cannot update or delete anything.

## Observability

Prometheus metrics on `:9000/metrics` (host port `9002` in compose):
per-detector tick counters, evaluation durations, emitted/suppressed signal
counts, failure counters.

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
