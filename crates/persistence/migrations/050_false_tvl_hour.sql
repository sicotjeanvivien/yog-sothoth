-- 050_false_tvl_hour.sql — one DAMM v2 TVL hour frozen on expired prices: false, not unmeasured, so it
-- leaves the series with its discoveries.
-- ⚠️ Under its bound only: any other reading of that hour stays. A fresh database has none.

WITH gone AS (
    DELETE FROM analytic_protocol_tvl
     WHERE protocol = 'meteora_damm_v2' AND bucket = '2026-10-31 10:00+00'
       AND tvl_usd < 32000000
    RETURNING protocol, bucket
)
DELETE FROM analytic_protocol_tvl_discovered d
 USING gone g
 WHERE d.protocol = g.protocol AND d.bucket = g.bucket;
