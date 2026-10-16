-- Seeds the watched-pools allowlist (`yog-migrate seed-watched-pools`): idempotent, never
-- deactivates a row. ⚠️ One pool only, SOL-USDC, whose rationale does not decay: a committed
-- "hot" list goes quiet and still looks healthy. Pick the rest fresh — see *Choosing pools
-- to watch* in the persistence README.

INSERT INTO watched_pools (pool_address, protocol, note) VALUES
    -- ⚠️ 'meteora_damm_v2', not 'damm_v2': `Protocol::from_str` refuses it and the indexer exits.
    ('8Pm2kZpnxD3hoMmt4bjStX2Pw2Z9abpbHzZxMPqxPmie',
     'meteora_damm_v2',
     'SOL-USDC — routing hub; the one pick whose rationale does not decay')
ON CONFLICT (pool_address) DO NOTHING;


-- Pools trading now: fill in from the README's ranking, then uncomment.
-- INSERT INTO watched_pools (pool_address, protocol, note) VALUES
--     ('<pubkey>', 'meteora_damm_v2', '<pair> — vol30m $<x>, TVL $<y>, <date>'),
--     ('<pubkey>', 'meteora_damm_v2', '<pair> — deep USDC quote, <date>'),
--     ('<pubkey>', 'meteora_damm_v2', '<pair> — thin, high turnover, <date>')
-- ON CONFLICT (pool_address) DO NOTHING;


-- What the indexer will subscribe to. ⚠️ Shown under `psql -f` only: the binary discards
-- these rows, and reads the allowlist back itself.
SELECT pool_address, protocol, added_at, note
FROM watched_pools
WHERE active = TRUE
ORDER BY added_at DESC;
