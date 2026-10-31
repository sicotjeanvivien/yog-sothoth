-- The price candidates and what tells their classes apart: known mints alive within $1 seconds.
WITH candidates AS (
    SELECT mint, fetched_at FROM token_metadata
    WHERE mint IN (
        SELECT mint FROM token_last_price
        WHERE fetched_at >= (SELECT LEAST(max(fetched_at), now()) FROM token_last_price)
                            - $1::BIGINT * INTERVAL '1 second'
        UNION ALL
        SELECT side.mint
        FROM pool_current_state pcs
        JOIN pools p ON p.pool_address = pcs.pool_address
        CROSS JOIN LATERAL (VALUES (p.token_a_mint), (p.token_b_mint)) AS side(mint)
        WHERE pcs.last_event_at >= now() - $1::BIGINT * INTERVAL '1 second'
    )
),
-- ⚠️ At the last price whatever its age: `pool_current_tvl` would make a stale mint worthless.
held AS (
    SELECT side.mint, SUM(yog_amount_usd(side.reserve, tm.decimals::INTEGER, tlp.price_usd)) AS usd
    FROM pool_current_state pcs
    JOIN pools p ON p.pool_address = pcs.pool_address
    CROSS JOIN LATERAL (VALUES (p.token_a_mint, pcs.reserve_a), (p.token_b_mint, pcs.reserve_b))
        AS side(mint, reserve)
    JOIN token_metadata tm ON tm.mint = side.mint
    JOIN token_last_price tlp ON tlp.mint = side.mint
    GROUP BY side.mint
)
SELECT c.mint AS "mint!: String",
       (NOT EXISTS (SELECT 1 FROM token_last_price tlp WHERE tlp.mint = c.mint)
        AND c.fetched_at < now() - $2::BIGINT * INTERVAL '1 second') AS "never_priced_past_grace!",
       COALESCE(h.usd < $3, FALSE) AS "below_low_value!"
FROM candidates c
LEFT JOIN held h ON h.mint = c.mint
