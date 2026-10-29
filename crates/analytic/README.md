# yog-analytic

The analytic writer. Every cycle — five minutes by default — it prices the
two tokens of every pool **once** per closed 5-minute tranche into
`analytic_pool_prices_5min`, and values that tranche's swaps at those prices
into `analytic_pool_5min`, then each of its liquidity moves into
`analytic_liquidity_moves`, summed per protocol and hour into
`analytic_protocol_liquidity_hourly`, and each claim of a position's fees or rewards into
`analytic_position_fee_claims` and `analytic_reward_claims`; draws the rotation runs from the tranches into
`analytic_rotation_runs` and `analytic_rotation_run_hours`; sums the tranches per pool and hour into
`analytic_pool_hourly`, and those facts per protocol into
`analytic_protocol_hourly`; counts the pools born per protocol and hour into
`analytic_protocol_births_hourly`, and the positions opened, locked, split and closed into
`analytic_protocol_position_changes_hourly`; measures the TVL of every protocol into
`analytic_protocol_tvl`, and the part of it the pools discovered in each window
hold into `analytic_protocol_tvl_discovered`; and freezes each hour ten minutes after its close,
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
before the end of the tranche (`yog_price_asof`), unless our pools contradict it
more than ten times over (`yog_market_believed`, 046): it then has none — its
price falls to what its pool traded, its swaps are valued by the other token, and
a move or a claim counts its side 0 beside a valued one, and is unvalued alone. The other token takes the
lower of its market price and its price in the pool: what the tranche traded,
`traded_b / traded_a` in token units, times the anchor's — never the pool's mid
price, which one swap sends to the end of its range on an empty pool. When both
are traded in as many pools, each takes the lower of its two prices; a token
without a market price takes its price in the pool. Without that rate — a
token's decimals unknown — there is nothing to bound by, and only the anchor
keeps a price.

Both prices are kept, `market_*` — the believed quote — beside the published
`price_*`: their gap is how far the pool and the market disagree, past a quote our
pools contradict, which `market_*` does not keep. The table covers the pools with
swaps in the tranche, and only them.

## The value of a swap

The volume is a flow: it is measured by what changed hands, not by a price given
to each token, and **gross**, fee included, as Meteora publishes it. Each
direction of a tranche is worth its **trusted leg** at market price — the
anchor's when it has a market price and decimals, else the other token's, the
lower of the two on a tie. A purchase of A with SOL is worth the SOL paid in. A
sale is worth the SOL paid out plus the fee the pool took on them: cp-amm always
returns an output net of its fee. A token emptied in its pool, or one no market
prices, is then worth what changed hands against it, whatever its price. The net
is the gross less the fees.

Which fee belongs to which direction follows cp-amm (`state/fee.rs`): A→B pays
on its output, in B; B→A pays in A in a BothToken pool, else on its input in B.
The 5-minute aggregate carries the buys' share of the B fees beside their total,
the rest being the sales'.

A fee is valued with the token it is taken in: at market for a trusted token;
otherwise as its share of the gross amount of the direction that paid it, times
that direction's trusted leg — amounts of one token, so its decimals never
matter. Its protocol and referral parts follow it, direction by direction.

A pool-tranche is valued when it has a trusted token and each direction and
each fee can be computed; otherwise it is counted unvalued.

## The value of a liquidity move

A move — a deposit, a withdrawal, or a pool's creation, which deposits its first liquidity
without a liquidity event — is worth its two amounts at its pool's prices in the tranche, the
prices just written for the swaps: what the pool traded bounds them, not the market. As in the
TVL, a token without a market price counts 0: one small trade's price never values a whole
stock deposited (a pool created with 10¹⁴ units of such a token was worth $5,751 billion by
its tranche's trade); a move none of whose moved tokens has a market price stays unvalued, and an
empty one is worth 0. Each move also says the lock its transaction puts on its position, or for a creation on a
position of its pool —
permanent, vesting or none, permanent first when it holds both — `NULL` for a move valued before 043. A deposit of a
token the market overprices is worth what the pool would pay for it — on the dev week,
$3.09 million of deposits in tranches with a swap, $19.41 million at market. In a
tranche where the pool did not swap, the anchor keeps its market price and the other
token takes the lower of its market price and the pool's last bounded one: never higher
than the pool last traded it, nor than the market now. A pool never priced in the last
seven days — the price table's — has no bound, and its move stays unvalued, counted.
Pools that only move liquidity stay out of the swaps' facts: they are not active pools.

## The value of a claim

A claim of a position's fees is worth its two amounts at the price a move of the same pool
would take in the tranche — the same rule, a token without a market price counting 0; an
empty claim, both amounts 0, is worth 0, priced or not: it took nothing. A reward in a token
of its pool takes that side's price under the same condition; a reward in another token, its
market price (`yog_price_asof`) at the end of the tranche, once its pool's mints are known:
unresolved, the reward may be one of them. Without a price or decimals,
`amount_usd` stays `NULL`, the claim counted unvalued. No backfill:
`analytic_tranches.claims` marks the tranches written with their claims, and the first of them
is when the claims started being measured.

## The rotation runs

A tranche carries its **rotation**: what its quoted side — SOL, USDC or USDT — traded over
its mean reserve, read in the raw swaps with the tranche; a pair with both or neither takes
the smaller of its two sides, and a pool whose mints are not resolved yet has none. A tranche is marked at `RUN_ROTATION` (1); a pool's marked
tranches no more than `RUN_GAP` (30 min) apart make one **rotation run**, from the start of the
first to the end of the last, worth every tranche of the pool between them, marked or not.
The thresholds and the quoted mints live in `core` (`domain/analytic/rotation_run.rs`), once.

Every cycle rewrites the rotation runs not yet settled. A rotation run is **extinct** once `RUN_GAP` has
passed its last marked tranche, and **settled** once its last hour is written whole too:
then it is final, written once with its volume per hour. Its volume is on the hourly facts'
base, a pool-hour counted only valued whole: the volume outside the rotation runs never falls below
zero. No backfill: `analytic_tranches.rotations` marks the tranches written with their
rotation, and the first of them is when the rotation runs started being measured. A rotation run longer
than the tranches' seven days starts at the first one still kept.

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
  frozen. It is the reserves of every pool observed since indexing began, each
  side at its token's stock price (`token_stock_price`, 045): its Jupiter price when
  under 15 minutes old, unless more than ten times the highest price our pools'
  trades gave the token over 7 days, its ceiling. A side without one counts 0, a
  pool with no side valued is counted apart (`pools_unvalued`). Each cycle keeps
  every token's ceiling (`analytic_token_pool_ceilings`) from what each tranche's
  trade implied for it (`pool_a_usd`, `pool_b_usd` of the price table), a trade
  that paid less than $1 for it witnessing nothing. The
  pools Meteora holds that never emitted an event since are out of it: a limit of
  observation, which grows the figure as dormant pools wake.
- **Discovered part**: with each TVL, for every window — 24 h, 7 and 30 days — the
  TVL of the pools first seen since the window's start point was last measured (its
  `from` when there is none) whose `initialize_pool` was not observed. Such a
  pool brings its whole stock in at once; a variation of the TVL leaves it out to
  tell a discovery from a deposit, while a pool born in the window is one.
- **Periods**: on a cycle that freezes, the distinct pools of each window — 24 h,
  7 and 30 days (`Window`, in `core`) — ending at every hour it freezes, so a
  previous window read after a catch-up finds its row; the tokens, at the last
  one only.

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
- a pool-tranche without a trusted token when it is valued stays unvalued for
  good, and its hour with it. It is counted (`valued = false`,
  `active_pools_valued`), never filled with a guess;

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
