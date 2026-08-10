-- ============================================================================
-- 008_cagg_refresh_below_retention.sql — the refresh was erasing the aggregate
-- ============================================================================
-- Fixes the finding of `.project` ticket 03 — though not the defect that ticket
-- describes. Its §2 claims a hole forms when the refresh does not run for 24 h.
-- Measured on 10 August 2026 (TimescaleDB 2.27.1, throwaway database, 1-day
-- chunks, hourly aggregate, 12 days of rows), the real behaviour is worse and
-- has no trigger at all:
--
--   after a full refresh ................................. 288 buckets
--   after drop_chunks(older_than => 5 days) .............. 288 buckets
--   after ONE refresh over [now-31d, now-1h] ............. 130 buckets
--
-- **The retention does not touch the aggregate. The REFRESH destroys it.**
--
-- `drop_chunks` logs an invalidation over the range it removes. A refresh is
-- invalidation-driven: when its window contains that invalidation it recomputes
-- the range from raw rows that no longer exist, and writes the result — which
-- is nothing. The materialized rows are deleted.
--
-- Our policies put the window exactly there: `start_offset` 31 days against a
-- `drop_after` of 30, on all four pairs.
--
-- ## How much, exactly — and why the chunk geometry does NOT save us
--
-- Raw chunks span 7 days and are dropped only once ENTIRELY older than
-- `drop_after`, so a single drop clears rows 30 to 37 days old depending on
-- alignment. That looks like it might spare us: with the wrong alignment the
-- youngest dropped row is 32 days old and a 31-day window never reaches it.
-- (Measured: a first version of the guard in `tests/cagg_retention.rs` stayed
-- green for exactly that reason.)
--
-- It does not spare us, because **retention runs daily**. A chunk is dropped at
-- the first run after its end crosses `drop_after`, so at that moment its
-- newest rows are between 30 and 31 days old — inside a window reaching back
-- 31. The overlap is not a lottery, it is one per chunk. Measured on the real
-- 7-day geometry: **2160 buckets → 2136, exactly 24 — one day of history lost
-- per chunk dropped**, so roughly one day in seven beyond the 30-day line, for
-- ever. Not a cap at 30 days; a comb of permanent holes past it.
--
-- Either way it is the inverse of what §13 promises:
--
--     durable history — survives the 30d retention drop on the raw swap
--     hypertable, holding hourly volume per pool indefinitely
--
-- It has never bitten only because the job scheduler has been off since
-- 16 June (ticket 03 §1, and `docker-compose.yml`).
--
-- ## The rule
--
--     start_offset  <  drop_after
--
-- A refresh must never look at a range whose raw rows may already be gone.
-- Counter-check on a fresh database, `start_offset` 3 days against a 5-day
-- retention: the 288 buckets survive, and stay at 288 across three further
-- refreshes — TimescaleDB answers "already up-to-date", the invalidation being
-- outside the window.
--
-- ## Why 29 days, and what it costs
--
-- Any row younger than `drop_after` is guaranteed present, so 29 against 30
-- leaves a full day of margin. The cost is the catch-up window after a
-- scheduler outage, which goes from ~31 days to ~29 — a bucket still has
-- 29 days to be materialized before its raw rows age out. That also disposes of
-- the ticket's §2: the tolerance was never 24 h, it is four weeks either way.
--
-- ⚠️ `001_baseline.sql:1664` still states the reasoning that produced the bug —
-- "start_offset spans the full 30d retention window (raw rows never live
-- longer)". Spanning the retention window is precisely the defect. Forward-only
-- means it cannot be corrected there; `migrations/README.md` carries the
-- pointer, and this header is its other end.
--
-- ## Why no aggregate is rebuilt
--
-- Only the four policies are replaced. No view is dropped, no cagg recreated —
-- so the free-rebuild window 007's header warns about is NOT consumed here.
-- `remove_continuous_aggregate_policy` + `add_continuous_aggregate_policy` were
-- checked to run inside a transaction and to roll back cleanly, unlike
-- `refresh_continuous_aggregate` (see §13's note on why the migrations declare
-- a policy instead of calling it).
--
-- The invariant is asserted by `tests/cagg_retention.rs`, in three forms: the
-- rule itself, read out of `timescaledb_information.jobs`; the behaviour the
-- rule exists for; and the destruction that SURVIVES it.
--
-- ## ☠️ What this migration does not, and cannot, fix
--
-- It constrains the scheduled policy. It does not constrain a refresh someone
-- types. The invalidations the policy now never reaches do not expire, so once
-- retention has dropped its first chunk, `refresh_continuous_aggregate(cagg,
-- NULL, NULL)` processes all of them at once and deletes every materialized
-- bucket whose raw rows are gone — measured at 2160 → 779 on a database with
-- THIS migration applied and the policy refresh already clean.
--
-- Before retention has ever dropped a chunk the same command is harmless, and
-- is the right way to capture a backlog accumulated while the scheduler was
-- off. `migrations/README.md` carries both sides and the query that tells you
-- which one you are on. Read it before running a backfill.
-- ============================================================================


-- ── swap volume + realized fees ─────────────────────────────────────────────
SELECT remove_continuous_aggregate_policy('meteora_damm_v2_swap_events_hourly',
    if_exists => true);
SELECT add_continuous_aggregate_policy('meteora_damm_v2_swap_events_hourly',
    start_offset      => INTERVAL '29 days',
    end_offset        => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');


-- ── liquidity added / removed ───────────────────────────────────────────────
SELECT remove_continuous_aggregate_policy('meteora_damm_v2_liquidity_events_hourly',
    if_exists => true);
SELECT add_continuous_aggregate_policy('meteora_damm_v2_liquidity_events_hourly',
    start_offset      => INTERVAL '29 days',
    end_offset        => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');


-- ── LP position fees claimed ────────────────────────────────────────────────
SELECT remove_continuous_aggregate_policy('meteora_damm_v2_claim_position_fee_events_hourly',
    if_exists => true);
SELECT add_continuous_aggregate_policy('meteora_damm_v2_claim_position_fee_events_hourly',
    start_offset      => INTERVAL '29 days',
    end_offset        => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');


-- ── farming rewards claimed ─────────────────────────────────────────────────
SELECT remove_continuous_aggregate_policy('meteora_damm_v2_claim_reward_events_hourly',
    if_exists => true);
SELECT add_continuous_aggregate_policy('meteora_damm_v2_claim_reward_events_hourly',
    start_offset      => INTERVAL '29 days',
    end_offset        => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');
