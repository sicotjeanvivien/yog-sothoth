-- ============================================================================
-- 014_swap_raw_sized_to_its_readers.sql — the raw swaps are kept one week, and
-- compressed a day after their chunk closes
-- ============================================================================
-- The swap hypertable moves from 7-day chunks, compressed 7 days after they
-- close and dropped at 30 days, to:
--
--     chunk_time_interval ........ 1 day
--     compress_after ............. 1 day
--     drop_after ................. 7 days
--     refresh start_offset ....... 6 days   (end_offset, schedule unchanged)
--
-- The 7/7/30 geometry was set when a day of swaps weighed a few hundred KB.
-- Indexing every DAMM v2 pool, a day weighs ~2.85 GB, and up to fourteen days
-- of it sat uncompressed — on disk and in every dump.
--
-- ## Why these numbers
--
-- The aggregates read the raw rows for a few hours only. The 7 days are not
-- what a reader needs: they buy the time to react when the refresh stops (job
-- scheduler down, workers exhausted, Postgres down). That time is **6 days**,
-- the refresh window — an hour pending longer is out of the policy's reach,
-- and lost for good once its raw rows are dropped a day later. The
-- materialization alarm of yog-signals is what tells you.
--
-- Compression waits one day after the chunk ends: that covers late arrivals
-- (a gRPC replay reaches back at most ~40 min), and keeps today and yesterday
-- uncompressed, which is what the first page of an active pool's swap history
-- reads. A quiet pool's first page reaches into compressed chunks, decompressing
-- only that pool's segment in each.
--
-- `start_offset` 6 days against `drop_after` 7 keeps the rule of
-- `008_cagg_refresh_below_retention.sql` — `start_offset < drop_after` — with
-- the same one-day margin.
--
-- ## ⚠️ Traps
--
--   * `set_chunk_time_interval` applies to chunks created from now on. The
--     chunk open when this runs keeps its 7 days until it closes.
--   * `001_baseline.sql` (the swap section of §12, and §13's comment on the
--     swap aggregate) and `008` still describe the swaps at 7 / 30 / 29 days.
--     For the swaps, this file is the one that holds.
--   * On a database older than 7 days, the first retention run after this
--     migration drops every swap chunk past that line at once, and the refresh
--     no longer reaches past 6 days: an hour not materialized by then is lost.
--     Before applying, `yog_cagg_materialization_backlog()` must show the
--     swap aggregate's `oldest_pending_at` younger than 6 days, or NULL.
--   * A bounded backfill of `meteora_damm_v2_swap_events_hourly` must stay
--     within **6 days**, not 29: over 7 days its raw rows are gone, and a
--     refresh over that range deletes the buckets (`migrations/README.md`).
--   * The policies are replaced through `remove_*` / `add_*_policy`, not
--     `alter_job`: `setup_roles.sql` revokes the job API from `PUBLIC`, and
--     `yog_migrate` with it.
--
-- No chunk is rewritten or recompressed here. No GRANT: no object is created.
-- ============================================================================

SELECT set_chunk_time_interval('meteora_damm_v2_swap_events', INTERVAL '1 day');

SELECT remove_compression_policy('meteora_damm_v2_swap_events', if_exists => true);
SELECT add_compression_policy   ('meteora_damm_v2_swap_events', INTERVAL '1 day');

SELECT remove_retention_policy('meteora_damm_v2_swap_events', if_exists => true);
SELECT add_retention_policy   ('meteora_damm_v2_swap_events', INTERVAL '7 days');

SELECT remove_continuous_aggregate_policy('meteora_damm_v2_swap_events_hourly',
    if_exists => true);
SELECT add_continuous_aggregate_policy('meteora_damm_v2_swap_events_hourly',
    start_offset      => INTERVAL '6 days',
    end_offset        => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');
