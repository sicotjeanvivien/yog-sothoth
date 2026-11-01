-- 050_false_tvl_hour.sql — the DAMM v2 TVL hour of 31 Oct. 10:00 UTC was frozen at 31.93 M$ between
-- two prices, against about 50 M$ on either side: false, not unmeasured, so it leaves the series.
-- ⚠️ Below 32 M$ only: on any other reading of that hour, this deletes nothing. A fresh database has none.

DELETE FROM analytic_protocol_tvl_discovered d
 USING analytic_protocol_tvl t
 WHERE t.protocol = d.protocol AND t.bucket = d.bucket
   AND t.protocol = 'meteora_damm_v2' AND t.bucket = '2026-10-31 10:00+00'
   AND t.tvl_usd < 32000000;
DELETE FROM analytic_protocol_tvl
 WHERE protocol = 'meteora_damm_v2' AND bucket = '2026-10-31 10:00+00'
   AND tvl_usd < 32000000;
