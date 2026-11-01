-- 050_false_tvl_hours.sql — DAMM v2 TVL hours frozen on expired prices: false, not unmeasured, so they
-- leave the series with their discoveries.
-- ⚠️ Each under its own bound only: any other reading of that hour stays. A fresh database has none.

WITH false_hour (bucket, below) AS (
    VALUES ('2026-10-27 10:00+00'::TIMESTAMPTZ, 41000000::NUMERIC),
           ('2026-10-27 21:00+00', 37000000),
           ('2026-10-30 21:00+00', 35500000),
           ('2026-10-31 10:00+00', 32000000)
),
gone AS (
    DELETE FROM analytic_protocol_tvl t
     USING false_hour f
     WHERE t.protocol = 'meteora_damm_v2' AND t.bucket = f.bucket AND t.tvl_usd < f.below
    RETURNING t.protocol, t.bucket
)
DELETE FROM analytic_protocol_tvl_discovered d
 USING gone g
 WHERE d.protocol = g.protocol AND d.bucket = g.bucket;
