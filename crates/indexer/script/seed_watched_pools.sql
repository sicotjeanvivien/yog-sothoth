-- ============================================================
-- yog-sothoth — Seed: watched_pools (dev)
-- Last updated: April 2026
--
-- Seeds the watched_pools allowlist with the 10 pools selected
-- from the 7-day swap_events activity distribution.
--
-- This script is NOT part of the migrations. It is intended for
-- development environments where the database may be reset often
-- (`docker compose down -v`). Production seeding will use user-
-- driven selection (v0.3).
--
-- Idempotent: ON CONFLICT DO NOTHING — safe to rerun without
-- overwriting manual changes to `active` or `note`.
--
-- Usage:
--   docker exec -i <container> psql -U yog -d yog_sothoth \
--     < scripts/seed_watched_pools.sql
-- ============================================================

INSERT INTO
    watched_pools (pool_address, protocol, note)
VALUES (
        'AKniRboGuKBRAUWh2QvQmMxDppcn8uzDx1LAngADJoBv',
        'meteora_damm_v2',
        'High activity — 906 swaps over ~50 min (2026-04-22)'
    ),
    (
        '8DW1L4yJRm2NNygASN1nFKEXwxLurkozxuYATZCT3gpb',
        'meteora_damm_v2',
        'High activity — 818 swaps over ~22 min (2026-04-22)'
    ),
    (
        '9g2wf7xTBsVxoVnypCdKrUmBtH6Ms1tSzVEJQNj86eHg',
        'meteora_damm_v2',
        'High activity — 774 swaps over ~10 min (2026-04-22)'
    ),
    (
        '5BohNRJgMtSv9C4PqxhvkXL1v1j7gouBoj4usNG8LGH',
        'meteora_damm_v2',
        'High activity — 758 swaps over ~22 min (2026-04-22)'
    ),
    (
        'GpnMyz78yTRiS2oBMroEKEynG7LkjWZq61aaU1MD558L',
        'meteora_damm_v2',
        'High activity — 720 swaps over ~35 min (2026-04-21)'
    ),
    (
        '6bkGH5bdNWym7eP2KKDDbCt5jMn9NB1dV7dN9fbb1Bz8',
        'meteora_damm_v2',
        'High activity — 674 swaps over ~10 min (2026-04-22)'
    ),
    (
        'CfpwKVuB8Y41re9U5qpYmD3oYiDijTcsHe3c3fs8GsFg',
        'meteora_damm_v2',
        'Extreme burst — 601 swaps in <1 min (2026-04-22)'
    ),
    (
        'AMxysMpo34c3aNb5bWW28p4AkXzWJFdM5Wdrtfmy4bMx',
        'meteora_damm_v2',
        'Edge case — ephemeral pool, 237 swaps in <1 min (2026-04-21)'
    ),
    (
        'EV9h8xS1yF3GJ8LnkaE65hQx5ViCSSeoVaHT6JPaVyPW',
        'meteora_damm_v2',
        'Edge case — ephemeral pool, 235 swaps over ~9 min (2026-04-21)'
    ),
    (
        '59drqEGrECHxMkHPKcr1JZggNfPxNKsrQP5MvCBEY5av',
        'meteora_damm_v2',
        'Edge case — ephemeral pool, 234 swaps over ~1 min (2026-04-21)'
    )
ON CONFLICT (pool_address) DO NOTHING;

-- Sanity check: should print 10 rows after a fresh seed.
SELECT
    pool_address,
    protocol,
    active,
    added_at
FROM watched_pools
WHERE
    active = TRUE
ORDER BY added_at DESC;