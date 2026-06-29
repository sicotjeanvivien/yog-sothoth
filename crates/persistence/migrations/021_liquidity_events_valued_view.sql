-- ============================================================================
-- 021 — meteora_damm_v2_liquidity_events_valued (VIEW)
-- ============================================================================
-- A read-time VIEW that adds a per-event trade-time USD value to each row of
-- `meteora_damm_v2_liquidity_events`, so the pool-detail liquidity table can
-- show a "Value (USD)" column. It exists to keep the heavy valuation SQL out
-- of the Rust string literals (the paginated read has TWO cursor paths —
-- forward and backward — so an inline join would be duplicated) and in
-- versioned SQL, the same move as migrations 019/020. The cursor query then
-- collapses to a slim `SELECT … FROM <view> WHERE pool_address = … ORDER BY …
-- LIMIT …` that the sqlx macro still verifies against the view's columns.
--
-- Valuation mirrors the established pattern (cf. 019): each leg's raw amount
-- divided by 10^decimals and priced at the most recent `token_prices` row
-- as-of the event's `timestamp` (the price WHEN it happened — trade-time, not
-- the current price, so historical rows don't drift). `value_usd` is NULL when
-- either token has no known price as-of the event, or the pool's mints /
-- decimals are not resolved yet (the NUMERIC arithmetic propagates NULL) —
-- "factual or absent, never fake"; the frontend renders such cells as "—".
--
-- The joins are LEFT (unlike 019/020's INNER): the event row must ALWAYS
-- appear in the table — only its `value_usd` is optional.
--
-- Not parameterized (a VIEW can't take args): it values every event of every
-- pool. The caller filters with `WHERE pool_address = $1 AND <cursor>` and
-- `LIMIT`; Postgres pushes those down, so a single page only runs the LATERAL
-- price lookups for the ~21 rows it returns (no materialization, planner-inlined).

CREATE VIEW meteora_damm_v2_liquidity_events_valued AS
SELECT
    le.pool_address,
    le.signature,
    le.timestamp,
    le.liquidity_event_kind,
    le.amount_a,
    le.amount_b,
    le.liquidity_delta,
    le.reserve_a_after,
    le.reserve_b_after,
    le.position,
    le.owner,
    (
        (le.amount_a::NUMERIC / POWER(10::NUMERIC, tma.decimals)) * tpa.price_usd
      + (le.amount_b::NUMERIC / POWER(10::NUMERIC, tmb.decimals)) * tpb.price_usd
    ) AS value_usd
FROM meteora_damm_v2_liquidity_events le
LEFT JOIN pools p ON p.pool_address = le.pool_address
LEFT JOIN token_metadata tma ON tma.mint = p.token_a_mint
LEFT JOIN token_metadata tmb ON tmb.mint = p.token_b_mint
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = p.token_a_mint AND fetched_at <= le.timestamp
    ORDER BY fetched_at DESC LIMIT 1
) tpa ON true
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = p.token_b_mint AND fetched_at <= le.timestamp
    ORDER BY fetched_at DESC LIMIT 1
) tpb ON true;

GRANT SELECT ON meteora_damm_v2_liquidity_events_valued TO yog_api;
