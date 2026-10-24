-- 037_pool_facts_read_by_api.sql — the api reads a pool's hourly facts (015) for its
-- `swaps` routes. Read only: yog_api writes nothing.

GRANT SELECT ON analytic_pool_hourly TO yog_api;
