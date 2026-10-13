# yog-analytic

The analytic writer. Every cycle — five minutes by default — it values the
swaps of the open hours **once**, per pool and per hour, into
`analytic_pool_hourly`; sums those facts per protocol into
`analytic_protocol_hourly`; measures the TVL of every protocol into
`analytic_protocol_tvl`; and freezes each hour ten minutes after its close.
A frozen hour is never written again.

It exists because valuing at read time does not scale: a reader of the
valuation VIEWs re-prices every (pool, hour) of its window on every request.
The writer prices each of them once, and the readers read sums.

It writes under the `yog_analytic` role, which may write the four `analytic_*`
tables and nothing else ([`crates/README.md`](../README.md#database-roles)).
No reader is wired to these tables yet.

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
        └── init.rs      ← init_database (one connection), init_heartbeat
```

One loop, so no `workers/` folder ([`crates/README.md`](../README.md#conventions)).
What a cycle writes is decided in `core` (`domain/analytic/`), how it is written
in `persistence` (`repositories/analytic.rs`).

## A cycle

`CycleWrite::plan(now, progress)` — pure, in `core`, tested on fixed clocks:

- **From** the hour after the last frozen one; before the first freeze, from
  the first hour ever written; on the very first cycle, from the current hour.
  Nothing before it is ever written: there is no backfill.
- **Through** the current hour, at most `MAX_HOURS_PER_CYCLE` (6) hours on.
  After an outage, the missed hours are caught up over the following cycles,
  oldest first, instead of in one long transaction.
- **Frozen**: every hour `H` of the range such that `H + 1 h + FREEZE_MARGIN`
  (10 min) has passed — the first cycle after that point freezes it. An hour
  freezes only on a cycle that rewrote it, so its last write sees everything
  that arrived within the margin.
- **TVL**: measured into the hour of the cycle, as long as that hour is not
  frozen.

`PgAnalyticRepository::write_cycle` writes it in **one transaction**. It first
locks `analytic_hours` against any other writer — readers go on — then lets
`CycleWrite::without_frozen` take out whatever was frozen since the cycle was
planned (see
[`persistence/README.md`](../persistence/README.md#the-analytic-tables-migration-015)).
A stop drops the cycle in flight, and rolls its transaction back with it; the
next start writes it again.

What the freeze costs, by design:

- a swap written after its hour froze — an indexer catching up after a long
  outage — is not in the facts;
- a token without a price, or without decimals, when its hour freezes leaves
  that pool-hour unvalued for good. It is counted (`valued = false`,
  `active_pools_valued`), never filled with a guess.

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
