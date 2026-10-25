-- 037_pool_facts_read_by_api.sql — the api reads a pool's hourly facts (015) and its
-- valued liquidity moves (035) for the routes under `/api/pools/{pool}`. Read only:
-- yog_api writes nothing.

GRANT SELECT ON analytic_pool_hourly TO yog_api;

-- A pool's moves, the most recent first: without it, every page reads the protocol's.
CREATE INDEX analytic_liquidity_moves_by_pool
    ON analytic_liquidity_moves (pool_address, timestamp DESC);
