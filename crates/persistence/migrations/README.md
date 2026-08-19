# Migrations — yog-persistence

Applied by the `yog-migrate` binary at startup of the Docker compose
stack, and manually during development via:

```sh
cargo run --bin yog-migrate -p yog-persistence
```

The connection string passed to `yog-migrate` must use the `yog_migrate`
role — the four runtime roles (`yog_indexer`, `yog_api`, `yog_context`,
`yog_signals`) intentionally cannot CREATE or ALTER tables.

## The baseline, and where the old numbers went

`001_baseline.sql` is the whole schema. It replaces the 42 files that built it
incrementally between June and August 2026, squashed on **5 August 2026** —
before the first production deployment, the only window in which a squash is
free. The next migration is `002_`.

`002_swap_implied_price.sql` — values a swap bucket by whichever of its two
tokens is priced, and is the one place where a cagg has been dropped and
rebuilt. That was free precisely because no cagg had ever materialized a bucket
(the job scheduler has been off since 16 June); it will not be free again once
the scheduler runs. A cagg cannot be `ALTER`ed, so any future column added to
one costs the same drop — and by then, a backfill.

`003_drop_pool_current_state_liquidity.sql` — drops `pool_current_state.liquidity`
and `last_liquidity_at`. The first named the pool's concentrated-liquidity **L**
and held a single position's unsigned `liquidity_delta`, last-write-wins; the
second was exact but read by nobody. **A DROP COLUMN is forward-only like any
other migration**: the data is not recoverable from the schema afterwards, so
the justification belongs in the file header, and it is there. Nothing was lost
here — the same magnitude stays on the liquidity event rows, next to the
add/remove kind that gives it meaning.

`004_fee_scheduler_params.sql` — six columns on the cp-amm satellite holding a
fee scheduler's decay curve, so a pool's base fee can be evaluated at read time
instead of being frozen on its genesis cliff (ticket 07, measured wrong by ×5
and ×49). They are NULL for every fee shape that has no time curve, and that
NULL is a decoded fact: `BaseFeeInfo` is 32 bytes the modes reinterpret, so the
same offsets under a market-cap scheduler or a rate limiter yield
plausible-looking nonsense. **No GRANT** — this satellite is granted at table
level, unlike `pools` whose `yog_context` rights are column-scoped.

`005_price_staleness_policy.sql` and `006_flow_valuation_completeness.sql` are
not described here: they redefine views (and, for 005, add the two price-age
functions) without changing any table. Their headers carry the reasoning, and
nothing this file could add would beat reading them.

`007_referral_fee_split.sql` — **the second drop and rebuild of the swap cagg**,
for two columns (`referral_fee_in_a` / `referral_fee_in_b`) that let the
realized fee be split into the three shares cp-amm actually applies (ticket 05:
the LP share was published as `fees − protocol`, which credits the referral to
the LPs). Still free, for the same reason 002 was — no bucket has ever been
materialized. ⚠️ **That is the point to carry forward, not the fix**: two
migrations have now paid nothing for a rebuild, and 002 had the window open in
August without adding these columns. The next one will need a backfill. A cagg
cannot be `ALTER`ed, so a column you can foresee wanting belongs in the rebuild
you are already doing.

☠️ **And read *`refresh_continuous_aggregate(cagg, NULL, NULL)` once retention
has run* below before you do that backfill.** The obvious way to run one is the
command that destroys the history you are trying to rebuild.

⚠️ **`001_baseline.sql` §13 still states the pre-007 rule as current fact** —
"so the LP share is `(fee_in_x - protocol_fee_in_x)`", in the header of this
very aggregate. It is wrong, and forward-only means it cannot be edited. That
matters more than a stale comment usually would: by this file's own argument
(*"a comment that says 'see migration 036' still resolves"*), the baseline is
where people go to read the current shape of an object — so the next reader of
the swap cagg meets, first, the formula 007 exists to remove. The correct rule
is in `007`'s header and in `crates/persistence/README.md` → *The realized fee
split*. This is the same defect the ticket is about, one level up: one
definition, written in two places, one of which went stale.

`008_cagg_refresh_below_retention.sql` — moves the four refresh policies from
`start_offset = 31 days` to `29`, against an unchanged `drop_after = 30 days`.
No view is touched and no aggregate rebuilt, so the window above is not spent.

**Verified against a live scheduler, which no test can do.** Every test and
every CI run applies migrations with `max_background_workers = 0`; production
applies them with the scheduler already ticking, and 008 *removes* four policies
whose jobs may be executing at that moment. Measured 10 August 2026:

- three fresh databases bootstrapped end to end with the scheduler at 8 workers
  and 30 live jobs on the neighbouring database — 8/8 migrations, four policies
  at 29 days, every time;
- then, on one of them, the four refresh jobs forced to `schedule_interval =
  1 second` over 20 days of seeded swaps, and 008 replayed **35 times**. The jobs
  really ran throughout (15–20 executions each, 0 failures) and every replay
  succeeded — 0 failures out of 35. The aggregate materialized 479 buckets.

So the deployment path is exercised, not assumed. That was the last unchecked
item of `.project` ticket 03.

The point was not to have fewer files. It was that the current shape of a table
had stopped being readable anywhere: `pools` had to be reconstructed by reading
001 + 014 + 015 + 018 + 027 + 036 + 037 + 038 and replaying the ADD/DROPs
mentally.

**A comment that says "see migration 036" still resolves.** The baseline is cut
into numbered sections, each naming the migrations it absorbs, and its header
carries the full old-number → section table. Grep the old number in
`001_baseline.sql`. The original files stay in git history
(`git log -- crates/persistence/migrations/`).

### How the squash was proved equivalent

Two fresh databases were built — one from the 42 migrations, one from the
baseline — and compared on **two** axes, because neither alone is sufficient:

1. `pg_dump --schema-only --schema=public`, with the order-dependent
   TimescaleDB internal ids normalised and the blocks sorted (pg_dump emits in
   dependency order, so the same schema built by two statement sequences yields
   permuted dumps);
2. the TimescaleDB catalog — `timescaledb_information.{hypertables, dimensions,
   compression_settings, continuous_aggregates, jobs}`. Retention, compression
   and refresh policies are catalog **rows**, not DDL, and pg_dump does not emit
   them.

Both empty. Mutation-tested in both directions: removing one retention policy
from the baseline produces **0** lines of pg_dump diff and 2 lines of catalog
diff; removing one column produces 2 and **0**. A dump-only comparison would
have been green while silently dropping every policy.

One deliberate difference was folded in — see below.

### ⚠️ What that comparison still cannot see: default privileges

Both oracle databases had `setup_roles.sql` applied, so `ALTER DEFAULT
PRIVILEGES` granted SELECT to the runtime roles on every new table and view —
and **an ACL cannot tell a default-privilege grant from an explicit one**. A
lost explicit `GRANT SELECT` is therefore invisible to the schema diff.

This is not hypothetical: migration 014 dropped and recreated `swap_events` and
`liquidity_events` without restating the grant 001 had given them, and nothing
noticed for two months. What caught it during the squash was
`tests/privileges.rs`, whose `sqlx::test` databases have no default privileges
— it pins the real matrix and it went red.

The grant is restored in the baseline (§14), which makes that section the one
place this file is not a pure squash. Folding it in rather than adding a `002_`
is defensible only because it happened **before the first deploy**, and only
because the privilege surface has its own assertion: the schema diff is blind
here, the matrix is not. After the first deploy the same finding would have to
go forward in a new migration.

Run the integration suite, not just a schema diff.

### ⛔ A database created before 5 August 2026: recreate it, do not resync

If your database was migrated by the 42-file chain, it holds a `_sqlx_migrations`
row for version 1 whose checksum is `001_initial_schema.sql`'s, plus versions 2
to 42 that no longer exist on disk. `yog-migrate` will refuse it —
`migration 1 was previously applied but has been modified` — and it is right to.

**The only correct action is to recreate the database.** Locally that is
`DROP DATABASE` then `yog-migrate -- bootstrap`; the data is disposable, which
is the whole reason the squash was allowed to happen at all.

⚠️ **Do not reach for the checksum-resync recipe below.** It is the first thing
this file offers a reader hitting a checksum error, and here it is actively
harmful: it would stamp the baseline's checksum onto a row recording a
*different* file, and leave versions 2-42 claiming migrations that the resolved
set no longer contains. The database would report itself up to date while its
recorded history describes a schema nobody can reproduce. That recipe exists for
one narrow case — a committed file edited in ways an applied database provably
never evaluated — and this is not it.

## The rule that binds a refresh policy to a retention policy

> **`start_offset` must be STRICTLY smaller than `drop_after`.**

A refresh must never look at a range whose raw rows may already be gone.
`drop_chunks` logs an invalidation over what it removes; a refresh is
invalidation-driven, so a window containing that invalidation recomputes the
range from rows that no longer exist and writes back the nothing it finds —
**deleting** the materialized buckets. The retention never touches the
aggregate; the refresh does.

⚠️ **The 7-day chunk geometry does not protect you.** A chunk is dropped only
once entirely older than `drop_after`, which *looks* like it keeps the dropped
range clear of a window that only overshoots by a day. It does not, because
retention runs **daily**: a chunk is dropped at the first run after its end
crosses the line, so its newest rows are then between 30 and 31 days old —
inside a 31-day window. Measured 10 August 2026 on the real geometry:
**2160 materialized buckets → 2136, exactly 24 — one day of history per chunk
dropped**, permanently, about one day in seven beyond the 30-day line.

Both directions are asserted by `tests/cagg_retention.rs`: the rule itself, read
out of the TimescaleDB catalog for all four pairs, and the behaviour it exists
for. ⚠️ `001_baseline.sql:1664` still carries the reasoning that produced the
bug — *"start_offset spans the full 30d retention window (raw rows never live
longer)"* — and forward-only means it cannot be corrected there.

### ☠️ `refresh_continuous_aggregate(cagg, NULL, NULL)` once retention has run

The rule above constrains **the scheduled policy**. It does not, and cannot,
constrain a refresh someone types. The invalidations the policy now carefully
never reaches do not expire — they accumulate in the invalidation log for ever —
so a full-range refresh processes all of them at once and deletes every
materialized bucket whose raw rows retention has dropped.

⚠️ **The precondition matters, and cuts the other way before it is met.** The
destruction needs invalidations, and those are logged by `drop_chunks`. On a
database where retention has never dropped a chunk — a fresh deploy, or any
database younger than `drop_after` — the log is empty and a full-range refresh
is **harmless and useful**: it is the only way to materialize history that
accumulated while the scheduler was off. Getting this backwards is its own
data loss: refuse the full refresh, enable the scheduler, and the retention job
drops raw rows that were never materialized and now never can be. So:

| state of the database | full-range refresh |
|---|---|
| retention has never dropped a chunk | **do it** — before enabling the scheduler |
| retention has dropped at least once | **never** — see below |

To tell which side you are on, ask the question that actually matters — **is
there materialized history older than the oldest surviving raw row?** — rather
than whether the retention job has run:

```sql
SELECT (SELECT min(bucket) FROM meteora_damm_v2_swap_events_hourly)         AS oldest_bucket,
       (SELECT time_bucket('1 hour', min(timestamp))
          FROM meteora_damm_v2_swap_events)                                 AS oldest_raw,
       (SELECT min(bucket) FROM meteora_damm_v2_swap_events_hourly)
         < (SELECT time_bucket('1 hour', min(timestamp))
              FROM meteora_damm_v2_swap_events)  AS a_full_refresh_would_destroy;
```

⚠️ **Do not use "has the retention job succeeded?" for this** — an earlier
version of this section did, and it was wrong in the direction the paragraph
above calls its own data loss. A retention run that finds nothing old enough
still *succeeds*: with the scheduler on, all four jobs report
`total_successes = 1` within a day, while the first chunk drop is thirty days
out. Measured on this dev database: that check answers **4** — "never run a full
refresh" — on a database whose oldest raw row is 2026-08-05 and where no chunk
has ever been dropped, i.e. exactly the one that should run it. The query above
answers `f` on the same database.

Measured 10 August 2026, on a database where migration 008 was already applied
and the policy's own refresh had just run clean:

| refresh | buckets |
|---|---|
| policy window (`29 days`) | **2160** — "already up-to-date" |
| then `NULL, NULL` | **779** |

**1381 buckets destroyed by one command**, on a correctly configured database.

This is not a hypothetical footgun: the paragraph on 007 above says the next
cagg rebuild *"will need a backfill"*, and a full-range refresh is the obvious
way to do one. It is the wrong way. A backfill must be run in **bounded slices
that stay inside the retention** — `CALL refresh_continuous_aggregate(cagg,
now() - INTERVAL '29 days', now() - INTERVAL '1 hour')` — and history older than
the raw retention cannot be rebuilt at all, because the rows it was computed
from are gone. That is the real cost of a late rebuild, and it is why columns
belong in the rebuild you are already doing.

`tests/cagg_retention.rs` pins this behaviour too, so the warning stays
falsifiable: if TimescaleDB ever stops recomputing over dropped ranges, that
test fails and this section can go.

## Watching the job scheduler run, locally

`docker-compose.yml` pins `timescaledb.max_background_workers = 0`, and it has
to stay there: `sqlx::test` creates a database per test and the scheduler races
the next test's DDL on the shared catalog. So no cagg had ever materialized a
bucket, and no policy had ever run — which is why every finding above went
unnoticed.

⚠️ **Read the price before running this.** Turning the scheduler on is not free
and not fully reversible:

- while it is on, the integration suite is **flaky** — that race is the reason
  for the pin, and it is the same one described under *What a local run cannot
  prove* below. Do not run tests until it is off again;
- the caggs will materialize, which **permanently closes the free-rebuild
  window** on that database: from then on, dropping and recreating an aggregate
  costs a backfill, with the caveat above that history older than the raw
  retention cannot be backfilled at all.

The restore step is therefore **not optional**, and it does not undo the second
point. With that understood:

```bash
cat > /tmp/scheduler-on.yml <<'YML'
services:
  postgres:
    command: ["postgres", "-c", "timescaledb.max_background_workers=8"]
YML
docker compose -f docker-compose.yml -f /tmp/scheduler-on.yml up -d postgres

# … observe, then PUT IT BACK before running any test:
docker compose up -d postgres
psql "$DATABASE_URL" -c "SHOW timescaledb.max_background_workers;"   # must be 0
```

What to look at — `timescaledb_information.job_stats` joined to `jobs` for
`total_runs` / `total_failures`, and the materialization watermark:

```sql
SELECT ca.view_name,
       to_timestamp(_timescaledb_functions.cagg_watermark(h.id) / 1000000.0)
  FROM timescaledb_information.continuous_aggregates ca
  JOIN _timescaledb_catalog.hypertable h
    ON h.table_name = ca.materialization_hypertable_name;
```

Done on 10 August 2026: all four refresh policies ran and succeeded on the first
scheduler tick, and the swap aggregate materialized **185 buckets** — the first
time any of them has. `claim_reward_events_hourly` stayed at `-infinity`, which
is correct: its raw table holds no rows. ⚠️ Note the consequence for the
free-rebuild window above — it is closed on any database where the scheduler has
now run, this dev one included.

## Forward-only

Migrations are forward-only. **A migration committed to git never
changes.** No `.down.sql` files; no edits to past migrations. If a
released migration introduced a problem, fix it forward by writing a
new migration that corrects the state.

**The one window where editing is admissible, and its price.** The rule
protects databases that have applied a migration and hold data you cannot lose.
Before the first production deploy that set is *only* developer machines, whose
data the project already treats as disposable. Editing is then a judgement
call, not a violation — but it is never free, and the cost falls where it is
least visible:

- CI recreates a database per run, so **CI stays green and proves nothing**;
- the failure surfaces only on machines that already applied it, later, as
  `migration N was previously applied but has been modified`, looking like a
  regression.

So an edit owes the team two things in the same commit: a statement of what
changed *in executable SQL* (ideally nothing), and the exact repair.

The repair re-syncs every recorded checksum with the file on disk. Prefer this
over a per-version `UPDATE`: it is idempotent, and it does not need to know
which migrations moved — which matters when two branches each touched a
different set.

```sh
python3 - "$DATABASE_URL_MIGRATE" <<'PY'
import hashlib, pathlib, re, subprocess, sys
stmts = []
for f in sorted(pathlib.Path("crates/persistence/migrations").glob("*.sql")):
    v = int(re.match(r"(\d+)_", f.name).group(1))
    c = hashlib.sha384(f.read_bytes()).hexdigest()
    stmts.append(f"UPDATE _sqlx_migrations SET checksum = decode('{c}','hex') WHERE version = {v};")
subprocess.run(["psql", sys.argv[1], "-q", "-c", "\n".join(stmts)], check=True)
PY
```

Only run it when the executable SQL is unchanged, or changed in a way an
already-migrated database provably never evaluated. Otherwise the row would
claim a statement ran that never did — recreate the database instead. **In
particular, never run it on a database predating the squash** — see the section
above. Prove it rather than assert it; comparing the files with comment lines
stripped is enough:

```sh
diff <(git show <before>:path/to/0NN_x.sql | grep -vE '^\s*--') \
     <(grep -vE '^\s*--' path/to/0NN_x.sql)
```

Done twice in August 2026, on files the baseline has since absorbed, to put
their headers into English (the repository's convention for migration bodies)
after they had merged. Both times the diff above came back empty or limited to
`RAISE EXCEPTION` strings that a successful run never evaluates.

The squash itself is the largest instance of this window ever taken, and it
closes it: from `002_` onwards there is production data behind the rule.

### A frozen comment can go stale, and the file cannot be the place it is fixed

Forward-only freezes the *comments* along with the SQL, so a header that
described the code accurately when it was written keeps describing it long
after the code moved on. That drift cannot be repaired in place — the fix
belongs here, where the reader can find it next to the file it corrects.

**`pools.fee_bps`, baseline §015.** The header still says the column is
*"unknown between a pool's discovery (swap/liquidity stream) and the arrival of
its InitializePool event, and left NULL if the fee blob ever fails to decode
(unknown BaseFeeMode)"*. Neither half holds since §036/§038: the
`InitializePool` event no longer writes this column, and **nothing decodes a fee
blob any more**. `fee_bps` is written by a single writer, yog-context, from the
on-chain `Pool` account, and it is NULL until that read happens.

The same file states the retired path a second time, and there as a *fact*
rather than as a nullability condition — §001, on the genesis event table:
*"the fee configuration is captured UNDECODED as a raw borsh blob
(`pool_fees_raw`). `pools.fee_bps` (§2) and the fee shape (§8) are derived from
these stored bytes."* The blob is still captured, and still undecoded; what is no
longer true is that anything is derived from it. Nothing reads it back. What schedules
that first read is the **NULL itself**: `list_unresolved` proposes any pool with
a NULL property column. `needs_refresh` (§038) is the other half — it schedules
a *re*-read after an event that invalidates a resolved value, since a pool that
already resolved would otherwise never be proposed again.

What in that header is still exact, and worth reading twice: *"for a
fee-scheduler (anti-sniper) pool this is the genesis cliff, not the live decayed
rate"*. The column is the floor at genesis. The fee a trader pays **now** is
derived at read time from the decoded curve (`base_fee_numerator_at`) and is a
different number — up to ×49 apart on a pool whose scheduler has expired, which
is what the audit measured against Meteora's own API.

**"Canonical (token_a, token_b) pool ordering", baseline §001 and §004** — on
the swap table, on `claim_protocol_fee_events`, and on `pool_current_state`
(*"Canonical reserves (token_a, token_b ordering as established in pools)"*). The order is real and consistent; the
word *canonical* is what misleads, because it reads as a normalisation. There is
none: `token_a` / `token_b` are the program's own designation, read off the
account and stored as-is, and **roughly a third of `pools` rows have
`token_a_mint > token_b_mint`**. The columns are safe to compare within a pool
and unsafe to use as a pair identity across pools. `MeteoraDammV2SwapEvent`
carries the full statement.

**"the column carries no `CHECK (> 0)`", `002_swap_implied_price.sql:241`** — and
its twin in `006_flow_valuation_completeness.sql`, the *"⚠️ One door stays open
here, deliberately"* paragraph, which spells out that `valuation_complete` tests
`price_usd IS NULL` rather than `NULLIF(price_usd, 0)` so *"a price rounded to
exactly zero yields `valuation_complete = TRUE` and a valuation of 0"*. **The
door is shut**: `009_price_positivity.sql` adds a validated
`CHECK (price_usd > 0)` on `token_prices.price_usd`, so no zero can be stored and
the flag cannot be reached through one.

Both paragraphs stay worth reading — they are still the clearest description of
*why* a zero is more dangerous than an absent price, and 006's is the rare case
of a header naming the gap it was leaving. Only their present tense is wrong.
Note that 006 is also where the measurement lives (*"0 such rows in 37 772"*),
remeasured at **0 in 49 980** on 12 August 2026 before 009 was applied — which is
what made the validating form of the constraint the safe one.

**"Very-high-supply memecoins live in exactly that regime",
`009_price_positivity.sql:19`** — and its variant *"live in precisely that
range — which is to say, the population this migration exists to rescue"* at
`002_swap_implied_price.sql:241-243`, on the same line as the stale `CHECK`
claim above. This is the stated *motivation* for the sub-`5e-19` guard, asserted
rather than measured, then copied from one file to the other. **They do not.** The guard is right; this
reason for it is wrong, and left standing it would make a future reader treat a
dormant defect as an imminent one — enough to take 009's manual deployment order
for an emergency.

A mint's `decimals` bounds *amounts*, not prices: a price is a ratio, with no
on-chain quantum (the Bitcoin-satoshi analogy does not transfer). What bounds a
price below is supply. Whole-token supply is **at most** `u64::MAX / 10^decimals`
(the table below is the reminder that real mints sit far under that ceiling), so
`price < 5e-19` implies a total valuation under `5e-19 × 1.8447e19 ≈ $9.22` —
and that is the extreme case, `decimals = 0`. At the pump.fun standard of 6 it
is `$9.2e-6`.

Measured against live Jupiter data on 12 August 2026, over the 205 mints its
search and recent-launch endpoints return:

| mint | decimals | supply | price USD | liquidity | FDV |
|---|---|---|---|---|---|
| BabyDoge | 1 | 2.96e17 | **3.47e-10** | $116k | $102M |
| SHIKOKU | 4 | 9.98e14 | 4.00e-10 | $34k | $400k |
| pump.fun floor | 6 | 1e9 | ~2e-6 | — | ~$2k |

The real population bottoms out around `1e-10` — **nine orders of magnitude
above the cliff**. BabyDoge would have to fall from $102M FDV to $0.15 to reach
it, and a token worth cents in total is precisely what Jupiter has no route for:
it returns no `usdPrice`, `into_fetched_price` drops it, and nothing is written.
This agrees with what 009 itself measured on the database (0 zeros in 49 980,
smallest `1.4e-6`) — the two sit thirteen orders of magnitude apart, and the
header treats that as luck rather than as structure.

What makes the guard worth keeping is stated correctly in
`TokenPrice::is_storable`, which is editable and now carries the full argument:
the reachable vector is the wire, not the market. `usd_price` is an unvalidated
JSON number from a third party, and `Decimal`'s maximum scale of 28 puts its
smallest positive value at `1e-28`: the whole band `[1e-28, 5e-19)` is
representable, passes any `> 0` test, and rounds to a stored zero. That is the
same justification as the column's *high* end, where the economics are more
absurd still (a token at `1e20` USD) and the `22003` outage no less real.

Two caveats, both on the evidence rather than the conclusion: 205 mints from
targeted queries establish an observed floor, not a proven one; and
`PriceProvider`'s two other provenances (`Helius`, `Fallback`) have no
`PriceSource` implementation today, so exactly one untrusted producer exists —
they argue for keeping the filter honest ahead of a future writer, not for a
second live input.

This is the right discipline for production safety:

- Reversing schema changes generally loses data anyway (a dropped
  column cannot be reconstructed).
- The hash of every applied migration is tracked in `_sqlx_migrations`;
  modifying a file would break every database that already applied it.
- "How do I roll back?" is answered by **backups** (pg_dump / Scaleway
  snapshots), not by reverse SQL. Before applying a fragile migration
  locally, run `pg_dump` first.

## Convention for new migrations

Each migration that creates a table emits its GRANT statements at the
end of the relevant section, in the same file. SELECT is covered by
the default privileges set in `setup_roles.sql`; everything else is
explicit per migration.

```sql
-- 002_new_event_table.sql

CREATE TABLE new_event_table (
    ...
);

CREATE INDEX ... ;

-- Grants — defaults cover SELECT for the three runtime roles; INSERT
-- / UPDATE goes here.
GRANT INSERT, UPDATE ON new_event_table TO yog_indexer;
```

The static structural grants (schema ownership, default privileges,
sequences default) live in
[`../src/bin/scripts/setup_roles.sql`](../src/bin/scripts/setup_roles.sql) —
next to the binary that embeds it, applied under the admin role by
`yog-migrate -- setup-roles`.

### An event table carries its position in the chain

Every `*_events` table carries three columns locating it in the chain, and one
uniform idempotency key — the shape migration 041 imposed, now `001_baseline.sql`
§12. A new event table is born with them:

```sql
CREATE TABLE meteora_<product>_<event_kind>_events (
    id                BIGSERIAL,
    pool_address      TEXT        NOT NULL,
    signature         TEXT        NOT NULL,
    -- … protocol-relevant columns …
    timestamp         TIMESTAMPTZ NOT NULL,
    slot              BIGINT      NOT NULL,
    event_index       INTEGER     NOT NULL,
    transaction_index BIGINT      NULL,
    PRIMARY KEY (id, timestamp)
);

-- Idempotency guard. `event_index` is what separates two events emitted by
-- ONE transaction — a route across several pools emits one per hop, under a
-- single signature and a single blockTime. Without it, `ON CONFLICT DO
-- NOTHING` silently keeps the first and drops the rest.
CREATE UNIQUE INDEX ON meteora_<product>_<event_kind>_events
    (signature, event_index, timestamp);
```

No `DEFAULT` on `slot` / `event_index`: an insert that forgets one must fail,
not inherit a plausible `0`. Widths follow the domain type, not the expected
magnitude — `event_index` is a `u16` so it is INTEGER, `transaction_index` a
`u32` so it is BIGINT (see `convert_i32_to_u16`'s doc-comment); both write
conversions are then total.

Do **not** invent a per-kind discriminant (`reward_index`, `second_position`
and friends). Those predate `event_index`, and migration 041 took them out of
the keys precisely to leave one rule.

### Name every index — do not let the server do it

`CREATE INDEX ON t (…)` leaves the name to Postgres, which has 63 bytes for it.
Our table names run to 53 characters, so the generated name gets truncated, and
when two truncate onto the *same* name Postgres appends `1`, `2`, … **in
creation order**. Two tables already do:

```
meteora_damm_v2_update_reward_duration_events  → …_signature_event_index_timesta_idx
meteora_damm_v2_update_reward_funder_events    → …_signature_event_index_timest_idx1
```

The `1` sits on `funder` only because `duration` is declared first. Add a third
table that truncates onto the same name and the suffix moves — a freshly
migrated database stops matching production, and nothing raises an error,
because nothing here is illegal.

So the rule for every migration from `010` on:

```sql
-- Name the index. Keep the name within 63 characters: on the longest table
-- today (53 characters) that leaves 10, so an event table's index is written
-- <table>_<short suffix>, not idx_<table>_<columns>.
CREATE UNIQUE INDEX meteora_<product>_<event_kind>_sig_uniq
    ON meteora_<product>_<event_kind>_events (signature, event_index, timestamp);

-- create_hypertable names one too — a default index on the time dimension, in
-- public. Turn it off and write that index out like any other.
SELECT create_hypertable('meteora_<product>_<event_kind>_events', 'timestamp',
                         chunk_time_interval => INTERVAL '7 days',
                         create_default_indexes => FALSE);
CREATE INDEX meteora_<product>_<event_kind>_ts
    ON meteora_<product>_<event_kind>_events (timestamp DESC);
```

And keep table names within **58** characters, because `<table>_pkey` is the one
index name that cannot be written by hand.

`src/bin/migrate/lint.rs` enforces all of that, DB-free — it belongs to
`yog-migrate`, the binary that owns these files:

```bash
cargo test -p yog-persistence --bin yog-migrate
```

**`001`–`009` are out of scope.** They break the rule 70 times over and stay as
they are — migrations are forward-only. Renaming those indexes is a separate job
needing its own migration with `ALTER INDEX … RENAME TO …`; nothing depends on
it today.

## The compression WARNINGs are expected

Enabling compression on an event hypertable makes TimescaleDB emit, once per
table:

```
WARNING:  column "id" should be used for segmenting or ordering
WARNING:  column "signature" should be used for segmenting or ordering
```

They appear in bulk during the integration suite, because `sqlx::test`
provisions a freshly migrated database per test and every `ALTER TABLE … SET
(timescaledb.compress, …)` warns again. Nothing is wrong.

**What it means.** Our unique indexes cover columns — `id` through the primary
key, plus `signature` and `event_index` — that are in neither
`compress_segmentby` (`pool_address`) nor `compress_orderby` (`timestamp`).
TimescaleDB warns that it cannot check those constraints against compressed
rows *cheaply*.

**It says "should", not "cannot": uniqueness is still enforced.** Verified
empirically on TimescaleDB 2.27 — inserting a duplicate into a compressed chunk
raises `duplicate key value violates unique constraint`, and the
`ON CONFLICT … DO NOTHING` idempotency guard behaves exactly as on an
uncompressed chunk. TimescaleDB decompresses the candidate segments to check.
So the warning is about **cost**, not correctness.

⚠️ *If you re-test this, use a fixed timestamp literal.* Two statements each
calling `now()` produce two different timestamps, so a
`(signature, event_index, timestamp)` index sees no conflict at all — a probe
written that way "proves" a hole that does not exist.

**Why we leave it alone.** The compression policy is 7 days and the indexer is
live: events carry a current timestamp and always land in the open, uncompressed
chunk. The expensive path is never taken. Putting `signature` into
`compress_segmentby` would silence the warning at the cost of destroying the
compression ratio — it is maximum-cardinality, so each segment would hold a
single row.

**When it would start to cost.** Backfilling events older than the compression
delay — every insert would decompress to check uniqueness. Correct, but slow.
Worth remembering the day historical replay becomes possible (see the gRPC
migration in the project tracker).

## ⚠️ What a local run cannot prove — compressed chunks

**A migration that passes locally has never met a compressed chunk, and cannot.**
The local Postgres runs with `timescaledb.max_background_workers = 0`
(`docker-compose.yml`, for the reason in `CLAUDE.md`), so the compression
policies never fire: **0 compressed chunk out of 45** on a typical dev
database. Production compresses at 7 days.

This is a structural blind spot, not an accident of timing — raising the worker
count re-introduces the job-scheduler race that made the integration suite
flaky. It matters because compressed chunks change what DDL and DML are allowed:
`ADD COLUMN`, `UPDATE`, and `CREATE UNIQUE INDEX` all have their own rules, and
a migration that touches rows (a backfill) is precisely the kind that could
fail there and nowhere else.

Verify by hand when a migration adds a column, backfills rows, or creates an
index — restore a database at the previous migration, compress its chunks, then
apply:

```sh
psql "$DATABASE_URL_MIGRATE" -c "
    SELECT compress_chunk(c)
    FROM show_chunks('meteora_damm_v2_swap_events') c;"
# then apply the migration and re-check:
psql "$DATABASE_URL_MIGRATE" -c "
    SELECT count(*) FROM timescaledb_information.chunks WHERE is_compressed;"
```

Done for migration 041 (August 2026): `ADD COLUMN`, the `row_number()` backfill
and `CREATE UNIQUE INDEX` all pass on a compressed chunk, the chunk stays
compressed, and the new unique index rejects a duplicate inserted into it — so
the constraint bites on compressed data too.

Three false greens have already come out of this project by trusting a check
that could not fail: an empty table, an unquantified loss, and this one. **A
verification that cannot come out red is not a verification.**

## Local development workflow

When you add a new migration:

1. Create `00X_xxx.sql` here.
2. Apply it locally:
   ```sh
   cargo run --bin yog-migrate -p yog-persistence
   ```
   (Reads `DATABASE_URL_MIGRATE` from `.env`; must be the yog_migrate
   role.)
3. Regenerate the `.sqlx/` offline metadata if you used the `query!`
   macros against the new schema:
   ```sh
   cd crates/persistence
   cargo sqlx prepare -- --all-targets --all-features
   ```
4. Commit the new migration AND the updated `.sqlx/` snapshot.

## Bootstrapping a fresh database

The database must already exist (`createdb`, or the compose volume); the binary
never creates one. Everything after that is one command:

```sh
cargo run --bin yog-migrate -p yog-persistence -- bootstrap
```

which is these three, in order:

| step | script | role |
|---|---|---|
| `setup-roles` | [`../src/bin/scripts/setup_roles.sql`](../src/bin/scripts/setup_roles.sql) — the five roles and the structural privileges | `DATABASE_URL_ADMIN` |
| `migrate` | this directory | `DATABASE_URL_MIGRATE` |
| `seed-watched-pools` | [`../src/bin/scripts/setup_watched_pools.sql`](../src/bin/scripts/setup_watched_pools.sql) — the startup allowlist | `DATABASE_URL_ADMIN` |

All three are idempotent, so `bootstrap` is safe to re-run — and safe against a
second database of the same cluster, where the roles already exist and only the
per-database privileges need applying. `bootstrap` checks both environment
variables before touching anything: failing on a missing `DATABASE_URL_MIGRATE`
*after* creating five cluster-wide roles leaves a half-provisioned cluster.

Then the runtime services (indexer / api / context / signals) can connect with
their respective roles — but check the allowlist first. It ships one pool, and
a pool-centric indexer collects only what it is subscribed to.