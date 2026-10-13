-- 016_analytic_read_by_api.sql — `/api/stats` reads the protocol aggregate and
-- the protocol TVL that yog-analytic writes (015).

GRANT SELECT ON analytic_protocol_hourly, analytic_protocol_tvl TO yog_api;
