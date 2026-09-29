-- ============================================================================
-- 013 — the oldest raw row each continuous aggregate has not materialised yet
-- ============================================================================
-- When the TimescaleDB scheduler stops, or a refresh policy keeps failing, the
-- hourly aggregates stop moving and nothing raises an error: from 16 June to
-- 10 August 2026 all four sat unmaterialised and nobody knew (see
-- `migrations/README.md`, *What a local run cannot prove*). `yog-signals` reads
-- this function on a timer and signals a dead man's switch from it.
--
-- One row per continuous aggregate, found in TimescaleDB's own catalog — an
-- aggregate added later is covered without touching this file or the code:
--
--   aggregate          the aggregate's view name;
--   watermark          where its materialisation ends; NULL while it has
--                      never materialised a bucket;
--   oldest_pending_at  the oldest raw row at or past the watermark — the one
--                      that has waited longest to be materialised. NULL when
--                      nothing is waiting.
--
-- Why the oldest PENDING row, and neither the clock nor the newest row:
--   - against the clock, an ingestion halt reads as a materialisation fault —
--     no bucket fills, so the watermark stops too;
--   - against the newest raw row, a table that stops receiving rows freezes
--     the lag at whatever it was. Measured here on 29 Sept 2026:
--     `claim_reward` holds six rows from 21 September that no refresh ever
--     materialised, and "newest row minus oldest" read 28 minutes, forever.
--   A row that waits is a refresh that did not run, whatever the indexer does.
--
-- `ORDER BY … LIMIT 1` rather than `min()`: the time index answers it from the
-- watermark onwards, and the ordered append over chunks stops at the first.
--
-- SECURITY DEFINER, so the caller needs EXECUTE and nothing else: it returns
-- two timestamps per aggregate, not rows. `search_path` is pinned, as every
-- SECURITY DEFINER function must. EXECUTE goes to `yog_signals` alone.
--
-- ⚠️ Assumes a TIMESTAMPTZ time dimension, which every hypertable here has:
-- `_timescaledb_functions.to_timestamp` reads the watermark as microseconds.
-- ============================================================================

CREATE FUNCTION yog_cagg_materialization_progress()
RETURNS TABLE (
    aggregate         TEXT,
    watermark         TIMESTAMPTZ,
    oldest_pending_at TIMESTAMPTZ
)
LANGUAGE plpgsql STABLE SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
DECLARE
    cagg RECORD;
BEGIN
    FOR cagg IN
        SELECT ca.view_name::TEXT AS view_name,
               ca.hypertable_schema::TEXT AS raw_schema,
               ca.hypertable_name::TEXT AS raw_table,
               h.id AS materialization_id,
               d.column_name::TEXT AS time_column
          FROM timescaledb_information.continuous_aggregates ca
          JOIN _timescaledb_catalog.hypertable h
            ON h.schema_name = ca.materialization_hypertable_schema
           AND h.table_name  = ca.materialization_hypertable_name
          JOIN timescaledb_information.dimensions d
            ON d.hypertable_schema = ca.hypertable_schema
           AND d.hypertable_name   = ca.hypertable_name
           AND d.dimension_number  = 1
         ORDER BY ca.view_name
    LOOP
        aggregate := cagg.view_name;

        watermark := _timescaledb_functions.to_timestamp(
            _timescaledb_functions.cagg_watermark(cagg.materialization_id));
        -- ⚠️ "Never materialised" is not `-infinity` on every version. Under
        -- 2.30.1 it is the lowest timestamptz Postgres represents,
        -- 4714-11-24 BC — measured on 29 Sept 2026 (`claim_reward`, raw
        -- rows present, no bucket ever materialised). `isfinite` alone lets
        -- that through as a real date.
        IF NOT isfinite(watermark)
           OR watermark <= '4714-11-24 00:00:00+00 BC'::TIMESTAMPTZ THEN
            watermark := NULL;
        END IF;

        -- The watermark is a bucket's END: a row at it belongs to the next
        -- bucket, the first one not materialised — hence `>=`.
        EXECUTE format(
            'SELECT %1$I FROM %2$I.%3$I WHERE $1 IS NULL OR %1$I >= $1 '
            'ORDER BY %1$I ASC LIMIT 1',
            cagg.time_column, cagg.raw_schema, cagg.raw_table)
           INTO oldest_pending_at
          USING watermark;

        RETURN NEXT;
    END LOOP;
END;
$$;

COMMENT ON FUNCTION yog_cagg_materialization_progress() IS
    'Per continuous aggregate: its watermark (NULL before a first bucket) and '
    'the oldest raw row not materialised yet (NULL when nothing waits). Read by '
    'yog-signals to tell a stalled materialisation from a stalled ingestion.';

REVOKE ALL ON FUNCTION yog_cagg_materialization_progress() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION yog_cagg_materialization_progress() TO yog_signals;
