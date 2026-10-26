-- 040_signals_filtered_reads_indexed.sql — a page of signals filtered by protocol or by
-- severity, newest first. Without them, a filter that matches few signals reads every chunk.
-- A compressed chunk is served by the Bloom filter TimescaleDB draws from each index at
-- compression (`timescaledb.auto_sparse_indexes`): one compressed before this file is
-- decompressed and compressed again to draw them.

CREATE INDEX signals_by_protocol ON signals (protocol, triggered_at DESC);
CREATE INDEX signals_by_severity ON signals (severity, triggered_at DESC);

DO $$
DECLARE
    c REGCLASS;
BEGIN
    FOR c IN
        SELECT format('%I.%I', chunk_schema, chunk_name)::REGCLASS
          FROM timescaledb_information.chunks
         WHERE hypertable_name = 'signals' AND is_compressed
    LOOP
        -- ⚠️ Not `compress_chunk(c, recompress => true)`: on a chunk already compressed,
        -- it draws neither filter.
        PERFORM decompress_chunk(c);
        PERFORM compress_chunk(c);
    END LOOP;
END $$;
