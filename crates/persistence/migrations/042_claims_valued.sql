-- 042_claims_valued.sql — every claim of a position's fees or rewards valued once by yog-analytic,
-- at its pool's price in the tranche, a reward outside the pool at its market price; the tranche
-- ledger says from when. No backfill: valued from the first tranche after this migration.

-- `amount_usd` is NULL exactly when the claim could not be valued.
CREATE TABLE analytic_position_fee_claims (
    signature    TEXT        NOT NULL,
    event_index  INTEGER     NOT NULL,
    timestamp    TIMESTAMPTZ NOT NULL,
    bucket       TIMESTAMPTZ NOT NULL,
    pool_address TEXT        NOT NULL,
    protocol     TEXT        NOT NULL,
    owner        TEXT        NOT NULL,
    amount_usd   NUMERIC,
    PRIMARY KEY (signature, event_index, timestamp)
);

SELECT create_hypertable('analytic_position_fee_claims', 'timestamp',
                         chunk_time_interval => INTERVAL '7 days',
                         create_default_indexes => FALSE);
CREATE INDEX analytic_position_fee_claims_by_protocol
    ON analytic_position_fee_claims (protocol, timestamp DESC);
-- ⚠️ A day past the longest window: a chunk is dropped whole, by its end.
SELECT add_retention_policy('analytic_position_fee_claims', INTERVAL '31 days');

CREATE TABLE analytic_reward_claims (
    signature    TEXT        NOT NULL,
    event_index  INTEGER     NOT NULL,
    timestamp    TIMESTAMPTZ NOT NULL,
    bucket       TIMESTAMPTZ NOT NULL,
    pool_address TEXT        NOT NULL,
    protocol     TEXT        NOT NULL,
    owner        TEXT        NOT NULL,
    mint_reward  TEXT        NOT NULL,
    amount_usd   NUMERIC,
    PRIMARY KEY (signature, event_index, timestamp)
);

SELECT create_hypertable('analytic_reward_claims', 'timestamp',
                         chunk_time_interval => INTERVAL '7 days',
                         create_default_indexes => FALSE);
CREATE INDEX analytic_reward_claims_by_protocol
    ON analytic_reward_claims (protocol, timestamp DESC);
SELECT add_retention_policy('analytic_reward_claims', INTERVAL '31 days');

ALTER TABLE analytic_tranches ADD COLUMN claims BOOLEAN NOT NULL DEFAULT false;

-- The price in the tranche of each of `pool_addresses`, at which a move or a claim is valued:
-- each passes its own pools, the anchor counting them with the tranche's swaps.
CREATE FUNCTION meteora_damm_v2_tranche_pool_prices(
    tranche        TIMESTAMPTZ,
    width          INTERVAL,
    pool_addresses TEXT[]
)
RETURNS TABLE (
    pool_address TEXT,
    price_a_usd  NUMERIC,
    price_b_usd  NUMERIC,
    market_a_usd NUMERIC,
    market_b_usd NUMERIC
)
LANGUAGE sql STABLE
BEGIN ATOMIC
WITH valued_pools AS (
    SELECT v.pool_address, p.token_a_mint::TEXT AS mint_a, p.token_b_mint::TEXT AS mint_b
      FROM unnest(pool_addresses) AS v (pool_address)
      LEFT JOIN pools p ON p.pool_address = v.pool_address
),
-- The anchor: the token in more pools of the tranche, these included; none on a tie.
pools_per_mint AS (
    SELECT m.mint, COUNT(*) AS pools
      FROM (SELECT s.pool_address, p.token_a_mint::TEXT AS mint_a, p.token_b_mint::TEXT AS mint_b
              FROM meteora_damm_v2_swap_events_5min s
              JOIN pools p ON p.pool_address = s.pool_address
             WHERE s.bucket = tranche
            UNION
            SELECT pool_address, mint_a, mint_b FROM valued_pools) t
     CROSS JOIN LATERAL (VALUES (t.mint_a), (t.mint_b)) m (mint)
     WHERE m.mint IS NOT NULL
     GROUP BY m.mint
),
swapped AS (
    SELECT q.pool_address, q.price_a_usd, q.price_b_usd, q.market_a_usd, q.market_b_usd
      FROM valued_pools mp
      JOIN analytic_pool_prices_5min q ON q.pool_address = mp.pool_address AND q.bucket = tranche
),
-- Without a swap: the anchor at market, the other at the lower of its market and the pool's
-- last bounded price. ⚠️ Never the pool's mid price; without a last price, no value.
unswapped AS (
    SELECT mp.pool_address,
           CASE WHEN n.anchor = 'a' THEN COALESCE(ma.price_usd, last_a.price)
                -- ⚠️ LEAST skips NULL: without a bound, the market alone would price the side.
                WHEN last_a.price IS NOT NULL THEN LEAST(ma.price_usd, last_a.price) END
               AS price_a_usd,
           CASE WHEN n.anchor = 'b' THEN COALESCE(mb.price_usd, last_b.price)
                WHEN last_b.price IS NOT NULL THEN LEAST(mb.price_usd, last_b.price) END
               AS price_b_usd,
           ma.price_usd AS market_a_usd,
           mb.price_usd AS market_b_usd
      FROM valued_pools mp
      LEFT JOIN LATERAL (
          SELECT q.price_a_usd AS price
            FROM analytic_pool_prices_5min q
           WHERE q.pool_address = mp.pool_address AND q.bucket < tranche AND q.price_a_usd IS NOT NULL
           ORDER BY q.bucket DESC
           LIMIT 1
      ) last_a ON true
      LEFT JOIN LATERAL (
          SELECT q.price_b_usd AS price
            FROM analytic_pool_prices_5min q
           WHERE q.pool_address = mp.pool_address AND q.bucket < tranche AND q.price_b_usd IS NOT NULL
           ORDER BY q.bucket DESC
           LIMIT 1
      ) last_b ON true
      LEFT JOIN pools_per_mint na ON na.mint = mp.mint_a
      LEFT JOIN pools_per_mint nb ON nb.mint = mp.mint_b
     CROSS JOIN LATERAL (
         SELECT CASE WHEN na.pools > nb.pools THEN 'a' WHEN nb.pools > na.pools THEN 'b' END
                    AS anchor
     ) n
      LEFT JOIN LATERAL yog_price_asof(mp.mint_a, tranche + width) ma ON true
      LEFT JOIN LATERAL yog_price_asof(mp.mint_b, tranche + width) mb ON true
     WHERE NOT EXISTS (SELECT 1 FROM swapped s WHERE s.pool_address = mp.pool_address)
       -- A pool never priced has no bound at all: what it moved or paid stays unvalued.
       AND (last_a.price IS NOT NULL OR last_b.price IS NOT NULL)
)
SELECT * FROM swapped
UNION ALL
SELECT * FROM unswapped;
END;

-- What two amounts of a pool are worth at its prices in a tranche; NULL when unvalued.
-- ⚠️ As the TVL: a side without a market price counts 0, never one small trade's price.
CREATE FUNCTION yog_pool_amounts_usd(
    amount_a     NUMERIC,
    amount_b     NUMERIC,
    dec_a        INTEGER,
    dec_b        INTEGER,
    price_a_usd  NUMERIC,
    price_b_usd  NUMERIC,
    market_a_usd NUMERIC,
    market_b_usd NUMERIC
)
RETURNS NUMERIC
LANGUAGE sql IMMUTABLE PARALLEL SAFE
RETURN CASE
    WHEN (amount_a > 0 AND market_a_usd IS NOT NULL) OR (amount_b > 0 AND market_b_usd IS NOT NULL)
    THEN CASE WHEN amount_a = 0 OR market_a_usd IS NULL THEN 0
              ELSE yog_token_amount(amount_a, dec_a) * price_a_usd END
       + CASE WHEN amount_b = 0 OR market_b_usd IS NULL THEN 0
              ELSE yog_token_amount(amount_b, dec_b) * price_b_usd END
END;

-- Written once: no UPDATE.
GRANT SELECT, INSERT ON analytic_position_fee_claims, analytic_reward_claims TO yog_analytic;
GRANT SELECT ON meteora_damm_v2_claim_position_fee_events, meteora_damm_v2_claim_reward_events
    TO yog_analytic;
GRANT SELECT ON analytic_position_fee_claims, analytic_reward_claims TO yog_api;
