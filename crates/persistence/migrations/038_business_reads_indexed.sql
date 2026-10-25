-- 038_business_reads_indexed.sql — the reads of the business routes: a protocol's pools
-- and a token's, on either side, newest first, and the tokens newest first. Without them,
-- a protocol or a token with few pools reads every pool, and a page of tokens sorts them all.
-- The api reads each token's latest price through `token_last_price` (017). Read only.

CREATE INDEX pools_by_protocol_first_seen
    ON pools (protocol, first_seen_at DESC, pool_address DESC);
CREATE INDEX pools_by_token_a ON pools (token_a_mint);
CREATE INDEX pools_by_token_b ON pools (token_b_mint);

CREATE INDEX token_metadata_by_fetched_at ON token_metadata (fetched_at DESC, mint DESC);

GRANT SELECT ON token_last_price TO yog_api;
