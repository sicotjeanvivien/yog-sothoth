-- ============================================================================
-- yog-sothoth — seed the watched-pools allowlist
-- ============================================================================
-- Run after the migrations:
--
--     cargo run -p yog-persistence --bin yog-migrate -- seed-watched-pools
--
-- (or by hand: psql "$DATABASE_URL_ADMIN" -f crates/persistence/src/bin/scripts/setup_watched_pools.sql)
--
-- Idempotent — `ON CONFLICT DO NOTHING`, safe to re-run after a partial seed or
-- against an existing database. It never deactivates or removes a row: curating
-- the allowlist stays a manual operation (see `README.md` → *Administration
-- helpers*).
--
-- ## Why this file exists at all
--
-- Under `INGEST_SCOPE=pools` — the local default — the indexer opens
-- one `logsSubscribe` per active row of `watched_pools`. With no active row it
-- exits on `NoSubscriptionTargets`, so this step is not optional; without it a
-- freshly recreated database gives an indexer that cannot start.
--
-- ## ⚠️ Why it holds ONE pool and not a top-ten
--
-- A committed list of "currently hot" pools is a trap this repository has
-- already paid for. On 4 August 2026, reseeding from the April-2026 selection
-- recorded in `README.md` produced an indexer that started, subscribed, logged
-- nothing abnormal — and collected nothing, because every pool in it had been a
-- burst that went quiet. A dead allowlist raises no error. It just looks
-- healthy.
--
-- So the only pool seeded here is the one whose justification does not decay:
--
--   **SOL-USDC** — the universal routing intermediary. It is also the single
--   most valuable pool to observe, because routed transactions are what
--   exercise `event_index`, the multi-hop path, and the same-slot ambiguity
--   counter (`yog_indexer_pool_current_state_same_slot_total`). A set of
--   isolated pools never triggers any of them.
--
-- Everything else must be picked fresh, at seeding time. That is the block
-- below, deliberately left commented out.
--
-- ## Picking the rest — rank on 30 minutes, not on 24 hours
--
-- Meteora's public API ranks pools, and its `volume` object is nested: the
-- 30-minute bucket is the one that says "trading right now", where `24h` can be
-- a burst that ended this morning.
--
--     curl -s "https://damm-v2.datapi.meteora.ag/pools?limit=60&order_by=volume24h&order=desc" \
--       | python3 -c "
--     import json, sys
--     rows = [(p['address'], p.get('name','?'), p['volume'].get('30m',0), p.get('tvl',0))
--             for p in json.load(sys.stdin)['data']]
--     for a, n, v30, tvl in sorted(rows, key=lambda r: -r[2])[:10]:
--         print(f'{a:<45}{n:<18}{v30:>12,.0f}{tvl:>12,.0f}')
--     "
--
-- Take the top of that ranking, plus — deliberately, rather than by rank — one
-- deep-TVL pool quoted in USDC and one thin, high-turnover pool. Their
-- valuation paths differ, and a seed made only of memecoin pairs leaves half
-- the read paths untested.
--
-- Record the 30-minute volume in `note`, so the next reader can tell at a
-- glance how old the selection is.
-- ============================================================================

INSERT INTO watched_pools (pool_address, protocol, note) VALUES
    -- ⚠️ `protocol` must be 'meteora_damm_v2'. `Protocol::from_str`
    -- (core/src/domain/protocol/model.rs) rejects 'damm_v2', and the indexer
    -- exits immediately on `invalid protocol: unknown program id`.
    ('8Pm2kZpnxD3hoMmt4bjStX2Pw2Z9abpbHzZxMPqxPmie',
     'meteora_damm_v2',
     'SOL-USDC — routing hub; the one pick whose rationale does not decay')
ON CONFLICT (pool_address) DO NOTHING;


-- ── Pools trading NOW — fill in from the ranking above, then uncomment ──────
--
-- INSERT INTO watched_pools (pool_address, protocol, note) VALUES
--     ('<pubkey>', 'meteora_damm_v2', '<pair> — vol30m $<x>, TVL $<y>, <date>'),
--     ('<pubkey>', 'meteora_damm_v2', '<pair> — deep USDC quote, <date>'),
--     ('<pubkey>', 'meteora_damm_v2', '<pair> — thin, high turnover, <date>')
-- ON CONFLICT (pool_address) DO NOTHING;


-- What the indexer will actually subscribe to on its next start.
--
-- ⚠️ Visible under `psql -f` only. `yog-migrate -- seed-watched-pools` runs this
-- file through `execute()`, which discards result rows — so this SELECT prints
-- nothing there. The binary reads the allowlist back through the repository and
-- logs it itself; this statement is for the by-hand path.
SELECT pool_address, protocol, added_at, note
FROM watched_pools
WHERE active = TRUE
ORDER BY added_at DESC;
