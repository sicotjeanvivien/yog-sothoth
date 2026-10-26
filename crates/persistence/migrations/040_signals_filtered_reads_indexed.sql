-- 040_signals_filtered_reads_indexed.sql — the two filters of a page of signals, protocol and
-- severity, indexed newest first. A compressed chunk is served by the Bloom filters drawn from
-- them at compression: one compressed before this file is decompressed and compressed again.

CREATE INDEX signals_by_protocol ON signals (protocol, triggered_at DESC);
CREATE INDEX signals_by_severity ON signals (severity, triggered_at DESC);

DO $$
DECLARE
    c REGCLASS;
BEGIN
    FOR c IN
        SELECT format('%I.%I', chunk_schema, chunk_name)::REGCLASS
          FROM timescaledb_information.chunks
         WHERE hypertable_schema = 'public' AND hypertable_name = 'signals' AND is_compressed
    LOOP
        -- ⚠️ Not `compress_chunk(c, recompress => true)`: on a chunk already compressed,
        -- it draws neither filter.
        PERFORM decompress_chunk(c);
        PERFORM compress_chunk(c);
    END LOOP;
END $$;
