# yog-analytic

The analytic writer. Every cycle — five minutes by default — it prices the
two tokens of every pool **once** per closed 5-minute tranche into
`analytic_pool_prices_5min`, and values that tranche's swaps at those prices
into `analytic_pool_5min`; sums the tranches per pool and hour into
`analytic_pool_hourly`, and those facts per protocol into
`analytic_protocol_hourly`; counts the pools born per protocol and hour into
`analytic_protocol_births_hourly`; measures the TVL of every protocol into
`analytic_protocol_tvl`; and freezes each hour ten minutes after its close,
writing then the distinct pools and the tokens of every window ending at it.
A tranche is valued once, a frozen hour never written again.

It exists because valuing at read time does not scale, nor holds: a reader of
the valuation VIEWs re-prices every (pool, hour) of its window on every
request, at a market price that can be an hour old and that the pool itself
contradicts. The writer prices each tranche once, and the readers read sums.

It writes under the `yog_analytic` role, which may write the `analytic_*` tables
and nothing else ([`crates/README.md`](../README.md#database-roles)).

## The price of a token in a pool

In each pool and tranche, the **anchor** is the token traded in more pools of
the tranche — SOL almost always. It keeps its market price, the last one at or
before the end of the tranche (`yog_price_asof`). The other token takes the
lower of its market price and its price in the pool: the pool's mid price at
the end of the tranche (from the last `next_sqrt_price`) times the anchor's.
When both are traded in as many pools, each takes the lower of its two prices;
a token without a market price takes its price in the pool. Without the pool's
rate — its raw swaps gone, or a token's decimals unknown — there is nothing to
bound by: only the anchor keeps a price, and the tranche is counted unvalued.
A token emptied in its pool is then worth what the pool pays for it, not its
price of before.

Both prices are kept, `market_*` beside the published `price_*`: their gap is
how far the pool and the market disagree. The table covers the pools with
swaps in the tranche, and only them.
`/api/protocols/…` reads the protocol aggregate and the TVL
(`repositories/protocol_activity.rs` in `yog-persistence`).

## Layout

```
analytic/src/
├── main.rs              ← init_rustls → dotenv → tracing → metrics → Config → Daemon → run
├── application/
│   ├── worker.rs        ← AnalyticWorker: the loop, one cycle per tick
│   ├── worker_tests.rs
│   ├── metrics.rs       ← yog_analytic_*
│   └── log.rs           ← the lines the worker writes, one function each
└── bootstrap/
    ├── config.rs        ← DATABASE_URL_ANALYTIC, ANALYTIC_CYCLE_SECS, ANALYTIC_HEARTBEAT_URL
    ├── daemon.rs        ← assembles the worker and runs it
    └── daemon/
        └── init.rs      ← init_db (one connection), init_heartbeat
```

One loop, so no `workers/` folder ([`crates/README.md`](../README.md#conventions)).
What a cycle writes is decided in `core` (`domain/analytic/`), how it is written
in `persistence` (`repositories/analytic.rs`).

## A cycle

`CycleWrite::plan(now, progress)` — pure, in `core`, tested on fixed clocks:

- **Tranches** from the one after the last valued, never inside a frozen hour,
  through the last that has been closed for `FREEZE_MARGIN` (10 min), at most
  `MAX_TRANCHES_PER_CYCLE` (72): the hours of `MAX_HOURS_PER_CYCLE`.
- **From** the hour after the last frozen one; before the first freeze, from
  the first hour ever written; on the very first cycle, from the current hour.
  Nothing before it is ever written: there is no backfill.
- **Through** the current hour, at most `MAX_HOURS_PER_CYCLE` (6) hours on.
  After an outage, the missed hours are caught up over the following cycles,
  oldest first, instead of in one long transaction.
- **Frozen**: every hour `H` of the range such that `H + 1 h + FREEZE_MARGIN`
  (10 min) has passed — the first cycle after that point freezes it. An hour
  freezes only on a cycle that rewrote it, and only once its last tranche is
  valued, so its last write sees every swap that arrived within the margin.
- **TVL**: measured into the hour of the cycle, as long as that hour is not
  frozen.
- **Periods**: on a cycle that freezes, for each window — 24 h, 7 and 30 days
  (`Window`, in `core`) — ending at the last hour it freezes. During a catch-up
  only that hour gets them: the API then has no previous window to read for the
  hours skipped.

## The periods

What a window cannot sum from its hours is written once per freeze:

- **The active pools** of a protocol, distinct: a pool active in two hours is
  one pool. Kept per window and hour, so the previous window is read, not
  recounted; every protocol of the domain gets its row, 0 included.
- **The tokens**: per token, the volume of the pools that hold it — a swap
  counts for both its tokens, as aggregators count it, so the tokens add up to
  twice the total — its distinct pools, and its pool-hours with the valued ones,
  the volume's coverage. Only the last freeze's are kept, the volume rounded to
  6 decimals.
- **The births** are not a period: counted per protocol and hour from its
  `initialize_pool` events, with the facts, and summed by the reader. A pool
  first seen is not a pool born: in `protocols` scope most pools seen are old.

All three cover every window, 30 days included, on whatever facts exist: the
API decides what a window may show (`crates/api/README.md`, *The measures*).

`PgAnalyticRepository::write_cycle` writes it in **one transaction**. It first
locks `analytic_hours` against any other writer — readers go on — then lets
`CycleWrite::without_written` take out whatever was valued or frozen since the
cycle was planned, and values the tranches one statement each (see
[`persistence/README.md`](../persistence/README.md#the-analytic-tables-migration-015)).
A stop drops the cycle in flight, and rolls its transaction back with it; the
next start writes it again.

What valuing once costs, by design:

- a swap written after its tranche was valued — more than ten minutes late, an
  indexer catching up after an outage — is not in the facts;
- a swap written into a 5-minute tranche already materialised is read only
  from the next refresh on: if its tranche is valued in between, it is not in
  the facts either
  ([`persistence/README.md`](../persistence/README.md#the-analytic-tables-migration-015));
- a token without a price, or without decimals, when its tranche is valued
  leaves that pool-tranche, and its hour, unvalued for good. It is counted
  (`valued = false`, `active_pools_valued`), never filled with a guess;
- the price in the pool is its mid price at the tranche's end: a token that
  falls within the tranche is valued at the low, one that rises at its market
  price — an error that can only undercount.

## Configuration

| Variable | Default | |
|---|---|---|
| `DATABASE_URL_ANALYTIC` | — | the `yog_analytic` role |
| `ANALYTIC_CYCLE_SECS` | `300` | time between two cycles, 1 to 3 600; the first runs at startup |
| `ANALYTIC_HEARTBEAT_URL` | — | a Healthchecks.io ping URL; optional, required by `docker-compose.prod.yml`. It is pinged once per cycle: the check's period follows `ANALYTIC_CYCLE_SECS` |

Each statement has two minutes before Postgres cancels it: a cycle stuck on a
lock fails with its reason rather than holding the next ones.

## Signals

- **Healthchecks.io**: a success after every written cycle, a `/fail` with the
  error after a failed one. A failed cycle is logged, counted and skipped; the
  next tick tries again. A stopped daemon is the silence the check notices.
- **Prometheus**, on `:9000/metrics` (host port 9003 in `docker-compose.yml`):
  `yog_analytic_cycles_total{outcome=written|failed}`,
  `yog_analytic_cycle_duration_seconds`, `yog_analytic_hours_frozen_total`,
  `yog_analytic_heartbeat_failures_total`.

## Deploying

1. A Healthchecks.io check, period 5 minutes, grace 10; its ping URL into the
   server's `.env` as `ANALYTIC_HEARTBEAT_URL`.
2. `DATABASE_URL_ANALYTIC` in the same `.env`, with a generated password.
3. **Before** deploying, create the role with that password, as admin:
   `CREATE ROLE yog_analytic LOGIN;` then `\password yog_analytic` in `psql`.
   `setup_roles.sql` leaves an existing role and its password untouched. Left
   to it, the role would get the `CHANGE_ME_analytic_password` placeholder,
   `yog-analytic` would restart until the password is changed, and
   `docker/update.sh` would fail its stability check on it.
4. Deploy (`docker/update.sh`).
