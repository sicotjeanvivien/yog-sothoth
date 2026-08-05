-- ============================================================================
-- 001_baseline.sql — the consolidated yog-sothoth schema
-- ============================================================================
-- This file is the whole schema. It replaces the 42 migration files that built
-- it incrementally between June and August 2026, squashed on 5 August 2026 —
-- before the first production deployment, the only window in which a squash is
-- free. From here the forward-only rule resumes: the next migration is `002_`,
-- and this file never changes again.
--
-- ## Why squash
--
-- Not to have fewer files — 42 files cost nothing to run. Because the current
-- shape of a table had stopped being readable anywhere: `pools` had to be
-- reconstructed by reading 001 + 014 + 015 + 018 + 027 + 036 + 037 + 038 and
-- replaying the ADD/DROPs mentally. What follows is the final state, with no
-- add-then-drop churn.
--
-- ## Equivalence is proved, not asserted
--
-- Two fresh databases were built — one from the 42 migrations, one from this
-- file — and compared on two axes, because neither one alone is sufficient:
--
--   1. `pg_dump --schema-only --schema=public` — tables, columns, constraints,
--      indexes (including their auto-generated names), comments, grants;
--   2. the TimescaleDB catalog (`timescaledb_information.{hypertables,
--      dimensions, compression_settings, continuous_aggregates, jobs}`) —
--      because retention, compression and refresh policies are catalog ROWS,
--      not DDL, and pg_dump does not emit them. A dump-only comparison would
--      have been green while silently dropping every policy.
--
-- Both diffs are empty. The procedure was itself mutation-checked in both
-- directions: removing one retention policy from this file yields 0 lines of
-- pg_dump diff and 2 of catalog diff; removing one column yields 2 and 0.
--
-- **One deliberate difference**, in §14: the `GRANT SELECT` on `swap_events`
-- and `liquidity_events` that migration 014 lost is restored here. It is the
-- one thing the comparison above cannot see — default privileges make a lost
-- grant indistinguishable from a held one — and the reason it is nonetheless
-- safe to fold in is that the whole privilege surface is asserted separately,
-- by `tests/privileges.rs`, against a database with no default privileges.
-- §14 carries the full account.
--
-- ## Where the reasoning went
--
-- Each section keeps the prose of the migrations it absorbs, and names them.
-- A comment elsewhere in the codebase that says "see migration 036" resolves
-- by grepping `036` in this file. The originals stay in git history.
--
--   old  section                              old  section
--   ---  -----------------------------------  ---  ----------------------------
--   001  §2 §3 §4 §5 §6 §7 §12 §14            022  §11
--   002  §12 (create_position)                023  §15
--   003  §12 (close_position)                 024  §15
--   004  §12 (lock_position)                  025  §15
--   005  §12 (permanent_lock_position)        026  §10
--   006  §12 (initialize_pool)                027  §8  (superseded by 036)
--   007  §12 (set_pool_status)                028  §12 (claim_protocol_fee)
--   008  §12 (update_pool_fees)               029  §12 (initialize_reward)
--   009  §12 (retention)                      030  §12 (fund_reward)
--   010  §13 (superseded by 014, 017)         031  §12 (withdraw_ineligible)
--   011  §13                                  032  §12 (update_reward_duration)
--   012  §13                                  033  §12 (update_reward_funder)
--   013  §13                                  034  §12 (withdraw_dead_liquidity)
--   014  §2 §13 §14                           035  §12 (split_position)
--   015  §2                                   036  §8
--   016  §2  (grant)                          037  §8  (column removed)
--   017  §13                                  038  §2
--   018  §8  (superseded by 036, 037)         039  §9
--   019  §15                                  040  §2 §4 §8 §9
--   020  §15                                  041  §12 §15
--   021  §15                                  042  §4
--
-- ## Conventions
--
--   - Each table emits its GRANT statements next to its definition. SELECT for
--     the runtime roles is covered by the ALTER DEFAULT PRIVILEGES in
--     `setup_roles.sql`; INSERT / UPDATE are always explicit here.
--   - Addresses are TEXT (base58 pubkeys), lossless u128 is NUMERIC(39, 0),
--     and integer widths follow the DOMAIN type rather than the expected
--     magnitude: a u16 is INTEGER (32 767 < 65 535), a u32 is BIGINT — so the
--     write conversion stays total instead of fallible.
--   - Object creation ORDER is load-bearing in one place, see §12.
-- ============================================================================

CREATE EXTENSION IF NOT EXISTS timescaledb;


-- ============================================================================
-- §2 — pools: the cross-protocol registry
--     [001 create, 014 mints become nullable, 015 fee_bps, 016 grant,
--      038 needs_refresh, 040 the referenced composite key]
-- ============================================================================
-- Generic registry of pools discovered from the transaction stream — it records
-- what was *seen*, not a watchlist. `protocol` is meaningful here and is part of
-- the row identity. A pool address is unique across protocols by Solana design
-- (PDA of program + seeds), so pool_address remains the PK.
--
-- What belongs here is what EVERY protocol has: an address, a protocol, a token
-- pair, a first/last sighting, and the normalized `fee_bps`. Anything shaped
-- like one protocol's vocabulary lives in that protocol's satellite (§8, §9) —
-- five columns accreted here before that rule was applied, and migration 036
-- moved them out.
--
-- ── token_a_mint / token_b_mint are nullable (014) ──────────────────────────
-- They used to be inferred per-event from the transferChecked CPIs in the
-- transaction. That heuristic is wrong on routed / multi-hop txs (a
-- Jupiter-style aggregator): the first transferChecked in the pre-event slice
-- can belong to another leg, so an ORE/USDC pool was recorded as SOL/SOL. The
-- authoritative source is the cp-amm Pool account (tokenAMint @ offset 168,
-- tokenBMint @ offset 200), decoded by yog-context — which resolves them
-- *after* discovery, hence nullable.
--
-- ── fee_bps (015) ───────────────────────────────────────────────────────────
-- The headline base fee in basis points. For cp-amm it is the base/cliff fee
-- numerator (leading u64 of the BaseFeeParameters blob) over FEE_DENOMINATOR
-- (1e9); for DLMM it is base_factor × bin_step × 10^base_fee_power_factor /
-- 10 000 (see §9). Semantically the same quantity in both: the FLOOR a swapper
-- pays, before any volatility-driven part — which is what makes it a genuine
-- cross-protocol notion rather than one protocol's field in a neutral table.
-- It is also a read surface: filtered (`WHERE fee_bps = $n`) and aggregated
-- (`list_fee_tiers`, `GROUP BY fee_bps`).
--
-- Nullable: unknown between a pool's discovery (swap/liquidity stream) and the
-- arrival of its InitializePool event, and left NULL if the fee blob ever fails
-- to decode (unknown BaseFeeMode) — skip-and-log, never a wrong value. For a
-- fee-scheduler (anti-sniper) pool this is the genesis cliff, not the live
-- decayed rate.
--
-- Unconstrained NUMERIC: bps = numerator / 100_000 is exact for any integer
-- numerator (including sub-bps fractions like 2.5), so no scale is imposed.
--
-- ── needs_refresh (038) ─────────────────────────────────────────────────────
-- Splits two roles that were tangled on the pool-property columns. yog-context
-- is a one-shot back-fill, not a synchroniser: `list_unresolved` only proposes
-- pools with at least one NULL column, so once a pool resolves it never comes
-- back. Anything that *changes over time* was therefore invisible to it, and
-- the indexer wrote those properties itself from the event stream — two writers
-- per table, and the indexer's writes failed with `permission denied`
-- silently for weeks (they are skip-and-log).
--
-- The indexer now stops writing property *values* and raises this flag instead;
-- yog-context re-reads the account and writes every property, remaining the
-- single writer. Reading the account also removes a class of decode hazard: an
-- account carries resolved state, while an update event carries a delta.
--
-- A flag on `pools` rather than NULLing the satellite column, for two reasons:
-- `pools` is where `yog_indexer` already holds UPDATE (NULLing a satellite
-- would need a fresh GRANT on that table, and one more per protocol as they
-- arrive), and the old value stays visible while the refresh is pending
-- instead of the dashboard showing "unknown" for a poll interval.
--
-- ── UNIQUE (pool_address, protocol) (040) ───────────────────────────────────
-- `pool_address` is already the primary key, so this adds no new guarantee
-- about `pools` — it exists because a foreign key needs a unique constraint
-- covering exactly its referenced columns, and three dependent tables (§4, §8,
-- §9) key on the pair so the schema itself says their protocol must be the one
-- the registry records.
--
-- It buys one real thing in the other direction: `pools.protocol` can no longer
-- change under a dependent row. Nothing updates that column today
-- (`PgPoolRepository::upsert` writes it on INSERT and its ON CONFLICT touches
-- only `last_seen_at`), so this blocks a bug rather than a workflow.

CREATE TABLE pools (
    pool_address  TEXT        PRIMARY KEY,
    protocol      TEXT        NOT NULL,
    token_a_mint  TEXT,
    token_b_mint  TEXT,
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    fee_bps       NUMERIC,
    needs_refresh BOOLEAN     NOT NULL DEFAULT FALSE,

    CONSTRAINT pools_pool_address_protocol_key UNIQUE (pool_address, protocol)
);

CREATE INDEX idx_pools_protocol     ON pools (protocol);
CREATE INDEX idx_pools_last_seen_at ON pools (last_seen_at DESC);

-- Partial index: the flag is FALSE for nearly every row nearly always, and the
-- only query that reads it wants the rare TRUE ones. A full index would be
-- mostly dead weight on a hot table.
CREATE INDEX idx_pools_needs_refresh
    ON pools (needs_refresh)
    WHERE needs_refresh;

COMMENT ON COLUMN pools.needs_refresh IS
    'Raised by the indexer when an event changes an account-derived property; '
    'cleared by yog-context after it re-reads the account. The indexer never '
    'writes property values itself — see migration 038.';

-- The asymmetry below is the lesson migration 036 paid for. `yog_indexer` holds
-- **table-level** UPDATE, so a column added later is covered the moment it
-- exists. `yog_context` holds **column-level** UPDATE, which by definition
-- never extends to a column added afterwards: adding a property column and
-- forgetting to restate the grant fails at runtime, under the real role only,
-- on a write path that is skip-and-log.
GRANT SELECT, INSERT, UPDATE ON pools TO yog_indexer;
GRANT SELECT                 ON pools TO yog_api, yog_context;
GRANT UPDATE (token_a_mint, token_b_mint) ON pools TO yog_context;  -- 014
GRANT UPDATE (fee_bps)                    ON pools TO yog_context;  -- 016
GRANT UPDATE (needs_refresh)              ON pools TO yog_context;  -- 038


-- ============================================================================
-- §3 — watched_pools: the startup allowlist  [001]
-- ============================================================================
-- Until the indexer runs on an upgraded RPC path (Helius transactionSubscribe
-- or gRPC/Geyser), ingestion is bounded to an allowlist of pools. The day the
-- allowlist is lifted the table stays — it just stops being read by the
-- indexer's startup filter.

CREATE TABLE watched_pools (
    pool_address TEXT        PRIMARY KEY,
    protocol     TEXT        NOT NULL,
    active       BOOLEAN     NOT NULL DEFAULT TRUE,
    added_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    note         TEXT
);

CREATE INDEX idx_watched_pools_active
    ON watched_pools (pool_address)
    WHERE active = TRUE;

GRANT SELECT ON watched_pools TO yog_indexer, yog_api;


-- ============================================================================
-- §4 — pool_current_state: the CQRS read model
--     [001 create, 040 composite FK, 042 the ordering key]
-- ============================================================================
-- One row per pool, whatever the protocol. Maintained event-driven by the
-- indexer: every persisted swap or liquidity event triggers an upsert, behind a
-- stale-write guard so reprocessing old events never overwrites a newer state
-- (see PgPoolCurrentStateRepository::upsert).
--
-- ── The guard stops ordering by the second (042) ────────────────────────────
-- It used to compare `last_event_at`, a TIMESTAMPTZ taken from `blockTime` — so
-- a **second**, strictly. But 56,1 % of swaps share their (pool, timestamp)
-- with another swap, up to 46 within one second: the audit of 3 August 2026
-- measured **33,5 % of state updates rejected** and labelled them "stale" as
-- though they were healthy concurrency. They were not — that was the guard's
-- own granularity. Most visible consequence: both legs of a routed transaction
-- are persisted, but the **first** won the projection, so the pool displayed
-- intermediate reserves and an intermediate sqrt_price, never the result.
--
-- `last_slot` / `last_event_index` / `last_transaction_index` carry the
-- position of the event that produced the state, and the guard compares them as
-- a tuple. `last_event_at` stays, as display data, but stops being the key.
--
-- The resulting order is PARTIAL, and measured rather than hidden:
-- `getTransaction` does not return `transaction_index` (see §12), so two
-- transactions of one slot touching the same pool are ranked on `event_index`
-- alone. That tie-break is not a coin flip — `event_index` numbers the
-- emissions of ONE transaction, so comparing it across two compares unlike
-- things, and within a slot the state converges to the largest index, which
-- systematically favours a leg deep inside a routed transaction over a
-- single-leg swap of the same block. What it buys in exchange is independence
-- from arrival order: the final state is a function of the SET of events, so a
-- replay reproduces it. Last-writer-wins would be unbiased and
-- non-deterministic. The case is counted:
-- `yog_indexer_pool_current_state_same_slot_total`.
--
-- The guard is written with `COALESCE(last_transaction_index, 0)` so the
-- gRPC/Geyser migration — where the transaction update carries its `index`
-- natively — makes the order total with no migration and no code change.
--
-- ── No index on last_event_at ───────────────────────────────────────────────
-- There was one from 001 to 042, and it supported nothing: no query orders or
-- range-filters on that column. It is written, then read as a scalar of a
-- single row fetched by primary key (`PoolCurrentStateLookup::get_by_address`).
-- It cost a write on every projection upsert, on the hottest write path there
-- is — and 042 made that path fire a third more often.

CREATE TABLE pool_current_state (
    pool_address           TEXT PRIMARY KEY,
    protocol               TEXT NOT NULL,

    -- Last event of any kind that touched this pool
    last_event_at          TIMESTAMPTZ NOT NULL,
    last_event_kind        TEXT        NOT NULL,
    last_signature         TEXT        NOT NULL,

    -- Canonical reserves (token_a, token_b ordering as established in pools)
    reserve_a              BIGINT NOT NULL,
    reserve_b              BIGINT NOT NULL,

    -- Price proxy: sqrt_price is updated by swap events only
    last_sqrt_price        NUMERIC(39, 0),
    last_swap_at           TIMESTAMPTZ,

    -- Liquidity (L): updated by liquidity events only
    liquidity              NUMERIC(39, 0),
    last_liquidity_at      TIMESTAMPTZ,

    updated_at             TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- The ordering key of the upsert guard (042). No DEFAULT: a write path that
    -- forgot one must fail loudly rather than inherit a plausible `0`.
    last_slot              BIGINT  NOT NULL,
    last_event_index       INTEGER NOT NULL,
    last_transaction_index BIGINT  NULL,

    CONSTRAINT pool_current_state_event_kind_valid
        CHECK (last_event_kind IN ('swap', 'liquidity_add', 'liquidity_remove')),

    -- The projection is cross-protocol by design, so there is no invariant here
    -- about WHICH pools may have a row. The invariant is that the two protocol
    -- labels agree. Safe for the ingestion hot path: the upsert's
    -- `ON CONFLICT … SET protocol = EXCLUDED.protocol` re-states the same value,
    -- and Postgres skips the referential check when the key is unchanged.
    CONSTRAINT pool_current_state_pool_fkey
        FOREIGN KEY (pool_address, protocol)
        REFERENCES pools (pool_address, protocol) ON DELETE CASCADE
);

COMMENT ON TABLE  pool_current_state IS
    'Per-pool projection of the latest known on-chain state, maintained by the indexer.';
COMMENT ON COLUMN pool_current_state.last_event_kind IS
    'Kind of the most recent event applied: swap | liquidity_add | liquidity_remove.';
COMMENT ON COLUMN pool_current_state.last_sqrt_price IS
    'Last observed sqrt_price (Q64.64 fixed-point as NUMERIC). NULL until first swap.';
COMMENT ON COLUMN pool_current_state.liquidity IS
    'Last observed liquidity L. NULL until first liquidity event.';

CREATE INDEX idx_pool_current_state_protocol
    ON pool_current_state (protocol);

GRANT SELECT, INSERT, UPDATE ON pool_current_state TO yog_indexer;
GRANT SELECT                 ON pool_current_state TO yog_api;


-- ============================================================================
-- §5 — network_status: the indexer's link to Solana  [001]
-- ============================================================================

CREATE TABLE network_status (
    -- Singleton guard: CHECK (id = 1) allows only one row.
    id              SMALLINT    PRIMARY KEY DEFAULT 1
                                CHECK (id = 1),

    slot            BIGINT      NOT NULL,
    rpc_latency_ms  INTEGER     NOT NULL,
    observed_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed the singleton row so the very first indexer write is a plain UPDATE path
-- and the API never has to handle an empty table.
INSERT INTO network_status (id, slot, rpc_latency_ms, observed_at)
VALUES (1, 0, 0, NOW());

GRANT SELECT, INSERT, UPDATE ON network_status TO yog_indexer;
GRANT SELECT                 ON network_status TO yog_api;


-- ============================================================================
-- §6 — token_metadata: near-immutable reference data per mint  [001]
-- ============================================================================
-- Populated by yog-context's metadata worker via Helius DAS.

CREATE TABLE token_metadata (
    mint              TEXT        PRIMARY KEY,
    symbol            TEXT,
    name              TEXT,
    decimals          SMALLINT    NOT NULL,
    logo_uri          TEXT,
    metadata_provider TEXT        NOT NULL DEFAULT 'helius_das',
    fetched_at        TIMESTAMPTZ NOT NULL,
    last_refresh_at   TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_token_metadata_last_refresh
    ON token_metadata (last_refresh_at);

GRANT SELECT, INSERT, UPDATE ON token_metadata TO yog_context;
GRANT SELECT                 ON token_metadata TO yog_api;


-- ============================================================================
-- §7 — token_prices: the USD price time series  [001]
-- ============================================================================
-- One row per (mint, fetch), populated by yog-context's price worker via
-- Jupiter V3. A hypertable, but deliberately NOT compressed: it is the join
-- target of every valuation VIEW in §15, always through a LATERAL "most recent
-- row as-of X" lookup, and there is no retention policy — prices are what make
-- historical rows readable at their trade-time value.

CREATE TABLE token_prices (
    mint           TEXT            NOT NULL,
    price_usd      NUMERIC(38, 18) NOT NULL,
    price_provider TEXT            NOT NULL,
    confidence     REAL,
    fetched_at     TIMESTAMPTZ     NOT NULL,
    PRIMARY KEY (mint, fetched_at)
);

SELECT create_hypertable(
    'token_prices',
    'fetched_at',
    chunk_time_interval => INTERVAL '7 days'
);

CREATE INDEX idx_token_prices_mint_recent
    ON token_prices (mint, fetched_at DESC);

GRANT SELECT, INSERT ON token_prices TO yog_context;
GRANT SELECT         ON token_prices TO yog_api;


-- ============================================================================
-- §8 — meteora_damm_v2_pool_properties: the cp-amm satellite
--     [018 + 027 as columns on `pools`, 036 moved them out, 037 dropped
--      partner_fee_percent, 040 the composite key]
-- ============================================================================
-- The pool properties that only exist for cp-amm, kept out of the
-- cross-protocol registry. "voie 3" forbids NULL columns for incompatible
-- fields on event tables; the rule was slow to reach `pools`, where five
-- cp-amm-shaped columns had accreted and would have been NULL for an entire
-- protocol once DLMM arrived.
--
-- ── protocol_fee_percent / referral_fee_percent (018) ───────────────────────
-- The cp-amm `Pool` account splits each swap's trading fee between the LPs and
-- payees, as integer percentages in PoolFeesStruct (right after the base_fee
-- blob whose cliff numerator feeds `pools.fee_bps`):
--   - protocol_fee_percent — Meteora's cut of the trading fee (byte 48)
--   - referral_fee_percent — a referrer's cut, only charged when the swap
--                            carries a referral account (byte 50)
-- yog-context decodes them from the same account fetch that resolves the mints
-- and fee_bps (PoolAccountWorker), at offsets verified against mainnet.
--
-- SMALLINT: the on-chain values are u8 (0..=100) and Postgres has no u8.
-- Nullable, and written as a unit — both are NULL together until that
-- resolution happens, or if the account ever fails to decode (skip-and-log).
--
-- ── The third percent had no referent (037) ─────────────────────────────────
-- `partner_fee_percent` decoded byte 49. That byte is not a partner fee, it is
-- padding:
--
--     pub struct PoolFeesStruct {
--         pub base_fee: BaseFeeStruct,      // 8..48
--         pub protocol_fee_percent: u8,     // 48
--         pub padding_0: u8,                // 49  ← what we were reading
--         pub referral_fee_percent: u8,     // 50
--     }
--
-- The word "partner" appears nowhere in cp-amm's `state/fee.rs`. The
-- surrounding offsets are correct, which is what made it hard to see. The data
-- agreed: partner_fee_percent = 0 for 971 pools out of 971, while
-- protocol_fee_percent and referral_fee_percent both decoded to 20 for the
-- same 971. A column that is always zero and always plausible. Pools with a
-- non-zero partner cut exist on Meteora; not one appeared, because nothing was
-- ever read.
--
-- ── base_fee_kind / has_dynamic_fee (027) ───────────────────────────────────
-- Where fee_bps is the headline fee *tier*, these capture how the fee
-- *behaves*, decoded from the raw borsh PoolFeeParameters blob
-- (initialize_pool_events.pool_fees_raw) by
-- `core::amm::damm_v2::decode_fee_config`.
--
--   base_fee_kind — how the base fee moves over time. One of:
--       'constant'              fixed fee, no scheduling
--       'scheduler_linear'      anti-sniper fee scheduler, linear decay
--       'scheduler_exponential' anti-sniper fee scheduler, exponential decay
--       'rate_limiter'          rate limiter / anti-sniper (mode 2)
--     Derived from the BaseFeeMode discriminant AND the scheduler period count:
--     a scheduler mode with zero periods is a constant fee, so the mode byte
--     alone is not enough.
--   has_dynamic_fee — whether a volatility-based dynamic fee sits on top of the
--     base fee (the Option<DynamicFeeParameters> tag is present). Orthogonal to
--     base_fee_kind: a pool can run a scheduler and a dynamic fee at once.
--
-- TEXT and no CHECK, on purpose: the closed value set lives in the Rust
-- `BaseFeeKind` enum (the single source of truth), and a Postgres enum would
-- force a second migration to extend it.
--
-- ── Why the protocol column is GENERATED (040) ──────────────────────────────
-- A `CHECK (protocol = 'meteora_damm_v2')` constrains the satellite row against
-- itself and never looks at `pools.protocol`, so on its own it enforces nothing
-- across the two tables. The linkage has to be the composite foreign key below.
-- `GENERATED ALWAYS AS (…) STORED` then does the pinning better than a CHECK
-- would: the constant IS the check, no INSERT anywhere has to supply it (not
-- one line of Rust changed), and Postgres refuses `INSERT … (pool_address,
-- protocol, …)` outright — a writer cannot lie about the protocol even
-- deliberately.
--
-- This is defence in depth, not a hole being plugged: the live write paths
-- already agree with the registry. What the constraint buys is what the Rust
-- guards cannot — they live in loops and call sequences no compiler protects,
-- and they hold only for TODAY's callers. The schema outlives the call graph.
--
-- For the third satellite (Orca, Raydium, whatever comes next): copy the
-- generated column and the composite FK into its own CREATE TABLE. That
-- replaces "remember to copy the Rust guard" with something the schema
-- enforces whether or not anyone remembered.

CREATE TABLE meteora_damm_v2_pool_properties (
    pool_address         TEXT     PRIMARY KEY,

    -- Fee-split percents (0..=100), resolved from the on-chain cp-amm `Pool`
    -- account by yog-context. Written as a unit — both are NULL together until
    -- that resolution happens.
    protocol_fee_percent SMALLINT,
    referral_fee_percent SMALLINT,

    -- Fee *shape*, decoded from the genesis PoolFeeParameters blob.
    base_fee_kind        TEXT,
    has_dynamic_fee      BOOLEAN,

    protocol             TEXT     NOT NULL
                                  GENERATED ALWAYS AS ('meteora_damm_v2') STORED,

    CONSTRAINT meteora_damm_v2_pool_properties_pool_fkey
        FOREIGN KEY (pool_address, protocol)
        REFERENCES pools (pool_address, protocol) ON DELETE CASCADE
);

-- yog-context owns this table: it resolves the cp-amm Pool account and writes
-- both the neutral pool columns and this satellite from the same read. A
-- generated column is writable by no role, so table-level grants are safe here.
GRANT SELECT, INSERT, UPDATE ON meteora_damm_v2_pool_properties TO yog_context;
GRANT SELECT                 ON meteora_damm_v2_pool_properties TO yog_api;


-- ============================================================================
-- §9 — meteora_dlmm_pool_properties: the DLMM satellite  [039, 040]
-- ============================================================================
-- The DLMM counterpart of §8: the pool properties that only exist for the
-- Liquidity Book product. Nothing below exists for cp-amm — there is no
-- `bin_step` in a constant-product pool and no fee scheduler in a bin-based
-- one — so a shared table would carry NULL columns for an entire protocol.
--
-- ## This table is dormant by construction, and that is not a bug
--
-- It stays EMPTY until DLMM event extraction lands. `MeteoraDlmm::extract_events`
-- is still a stub returning an empty outcome, and pool discovery runs off
-- extracted events (`pool_maintenance`), so no row with `protocol =
-- 'meteora_dlmm'` reaches `pools` — and this satellite's queue has nothing to
-- resolve. It was laid down ahead of that deliberately: the decoder, the
-- resolver and the read path are testable today (see
-- `crates/core/tests/fixtures/dlmm/accounts/`, and seeding one `pools` row by
-- hand resolves it end to end).
--
-- If you are reading this because the table is empty: that is why. Look at the
-- extractor, not here.
--
-- ## Configuration, not state
--
-- Every column is a *parameter*, changed only by an `update_fee_parameters`.
-- The pool's state — `active_id` (the active bin) and the volatility
-- accumulator with its decay — moves on every swap and belongs to
-- `pool_current_state`. Storing it here would rewrite this row on every crossed
-- bin, for a table whose whole point is that it rarely changes.
--
-- ## This is what makes `pools.fee_bps` genuinely cross-protocol
--
--   base_fee_rate = base_factor × bin_step × 10 × 10^base_fee_power_factor (1e9)
--   fee_bps       = base_factor × bin_step × 10^base_fee_power_factor / 10 000
--
-- See `yog_core::amm::dlmm::base_fee_bps`, and the nine real accounts in
-- `crates/core/tests/fixtures/dlmm/accounts/` that pin it to published tiers
-- (0, 1, 5, 25, 50, 100, 200 bps).

CREATE TABLE meteora_dlmm_pool_properties (
    pool_address               TEXT     PRIMARY KEY,

    -- Price increment between adjacent bins, in bps: bin i sits at
    -- (1 + bin_step / 10_000)^i. The defining property of a DLMM pool — the
    -- analogue of a fee tier — and one of the two inputs to the base fee.
    bin_step                   INTEGER,   -- u16

    -- The other two inputs to `pools.fee_bps`, kept raw so the derivation stays
    -- auditable and recomputable rather than only surviving as its result.
    base_factor                INTEGER,   -- u16
    base_fee_power_factor      SMALLINT,  -- u8

    -- Dynamic-fee parameters. There is no boolean here on purpose: DLMM has no
    -- `has_dynamic_fee` flag, it expresses "no dynamic fee" as
    -- variable_fee_control = 0, so the magnitude carries both facts.
    --   variable_fee_rate = ceil(variable_fee_control
    --                            × (volatility_accumulator × bin_step)² / 1e11)
    variable_fee_control       BIGINT,    -- u32
    max_volatility_accumulator BIGINT,    -- u32

    -- Meteora's cut of the trading fee, in **basis points** — not the whole
    -- percent cp-amm's `protocol_fee_percent` uses. The two are not comparable
    -- without scaling, which is one more reason they live in separate tables.
    protocol_share             INTEGER,   -- u16

    protocol                   TEXT     NOT NULL
                                        GENERATED ALWAYS AS ('meteora_dlmm') STORED,

    CONSTRAINT meteora_dlmm_pool_properties_pool_fkey
        FOREIGN KEY (pool_address, protocol)
        REFERENCES pools (pool_address, protocol) ON DELETE CASCADE
);

-- Every column is NULL together, for a pool discovered but not yet enriched:
-- they all come from one read of one `LbPair` account. Unlike cp-amm's
-- `base_fee_kind`, none has a partial-failure mode — they are fixed-offset
-- integers with no open enum to recognise — so `bin_step IS NULL` is an exact
-- test for "never resolved", and that is what the resolver's queue keys off.

GRANT SELECT, INSERT, UPDATE ON meteora_dlmm_pool_properties TO yog_context;
GRANT SELECT                 ON meteora_dlmm_pool_properties TO yog_api;


-- ============================================================================
-- §10 — announcements: operator → users, without a deploy  [026]
-- ============================================================================
-- An announcement (maintenance, incident, release, beta) must be publishable
-- WITHOUT a deploy — hence a table served by the api, not static web content.
-- The changelog page is the static counterpart; a 'release' announcement points
-- at it via link_url.
--
-- Deliberately NOT a hypertable: an operator-curated table of a handful of rows
-- with no time-series semantics — it joins the generic single-table family
-- (network_status), not the event family.
--
-- Severity deliberately does NOT reuse the Signal Engine scale (§11): a signal
-- severity is a detector's *business* conclusion (escalation semantics, dedup);
-- an announcement severity is the operator's *editorial* display choice. Three
-- same-named tags is vocabulary coincidence, not concept identity — each side
-- keeps its own enum and CHECK.
--
-- Publication is an operator INSERT/UPDATE via psql (admin); the authenticated
-- write endpoint is deferred to auth (v0.3). No runtime role gets write access
-- — yog_api stays read-only by design.

CREATE TABLE announcements (
    id         BIGSERIAL PRIMARY KEY,
    kind       TEXT        NOT NULL,   -- what it is about (label chip on the web)
    severity   TEXT        NOT NULL,   -- how prominently to display it
    message    TEXT        NOT NULL,   -- free operator text (English, v1 decision)
    link_url   TEXT,                   -- optional target, e.g. /changelog#v0.1.1
    starts_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ends_at    TIMESTAMPTZ,            -- NULL = shown until the operator closes it
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT announcements_kind_valid
        CHECK (kind IN ('maintenance', 'incident', 'release', 'beta')),
    CONSTRAINT announcements_severity_valid
        CHECK (severity IN ('info', 'warning', 'critical')),
    CONSTRAINT announcements_window_valid
        CHECK (ends_at IS NULL OR ends_at > starts_at)
);

-- The active-window read scans a handful of rows — no index beyond the PK.

GRANT SELECT ON announcements TO yog_api;


-- ============================================================================
-- §11 — signals: the Signal Engine's output  [022]
-- ============================================================================
-- A signal is a *conclusion* — a uniform shape across protocols — NOT a raw
-- on-chain event. It therefore does NOT follow the per-protocol "voie 3" table
-- split; it joins the generic single-table family (pools / token_prices): ONE
-- table, `protocol` is a column, discriminated additionally by `detector` (the
-- rule that fired).
--
-- Level-1 schema (decided 1 July 2026): typed common columns only, NO JSONB. If
-- a detector ever needs a structured payload it escalates via a nullable
-- `details JSONB` (level 2) or an extension table joined by id (level 3) —
-- never before a real detector proves the need.
--
-- Hypertable on `triggered_at`, like token_prices. Signals are the product's
-- valuable, sparse output: NO retention policy (kept indefinitely); compression
-- only kicks in after 30d.

CREATE TABLE signals (
    id            BIGSERIAL,
    detector      TEXT        NOT NULL,   -- e.g. 'flow_imbalance', 'tvl_drain'
    protocol      TEXT        NOT NULL,   -- e.g. 'meteora_damm_v2'
    pool_address  TEXT        NOT NULL,

    severity      TEXT        NOT NULL,
    value         NUMERIC     NOT NULL,   -- the metric that crossed the threshold
    threshold     NUMERIC,                -- the threshold crossed (traceability)
    message       TEXT,                   -- optional human-readable summary

    triggered_at  TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (id, triggered_at),       -- triggered_at must be in the PK (hypertable)

    -- DB-level safety net, same spirit as swap_events' trade_direction CHECK.
    CONSTRAINT signals_severity_valid
        CHECK (severity IN ('info', 'warning', 'critical'))
);

SELECT create_hypertable('signals',
    'triggered_at', chunk_time_interval => INTERVAL '7 days');

-- Per-pool feed (pool detail page) and per-detector feed (dashboard filter).
CREATE INDEX ON signals (pool_address, triggered_at DESC);
CREATE INDEX ON signals (detector,     triggered_at DESC);

ALTER TABLE signals SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'triggered_at DESC',
    timescaledb.compress_segmentby = 'detector'
);
SELECT add_compression_policy('signals', INTERVAL '30 days');
-- No add_retention_policy: signals are sparse and are the product output.

-- The signal engine writes; the api reads for the feed. Signals are append-only
-- conclusions: INSERT, never UPDATE. SELECT is granted to yog_signals too so a
-- detector can dedup against its own recent emissions.
GRANT SELECT, INSERT ON signals                 TO yog_signals;
GRANT USAGE, SELECT  ON SEQUENCE signals_id_seq TO yog_signals;
GRANT SELECT         ON signals                 TO yog_api;


-- ============================================================================
-- §12 — the 19 Meteora DAMM v2 event tables
--     [001 ring 1, 002-008 ring 2, 028-035 protocol fees / rewards / split,
--      009 differentiated retention, 041 slot + event_index + the unique key]
-- ============================================================================
-- One table per (protocol, event_kind) — "voie 3". Each holds only the columns
-- relevant to its event: no NULL columns for incompatible fields, no JSONB
-- blob. Cross-protocol reads go through the VIEWs in §14.
--
-- ## Every event table carries its position in the chain (041)
--
--     slot              BIGINT      NOT NULL   -- the slot it was observed in
--     event_index       INTEGER     NOT NULL   -- its rank among the tx's self-CPIs
--     transaction_index BIGINT      NULL       -- the tx's rank within the slot
--
-- and one uniform idempotency key, `(signature, event_index, timestamp)`.
--
-- The key used to be `(signature, timestamp)` — no intra-transaction
-- discriminant — while the INSERT is `ON CONFLICT DO NOTHING`. A transaction
-- routed across several pools emits one event per hop, so everything but the
-- first to land was dropped: no error, no log, no metric. Measured on 4 August
-- 2026 across three mainnet pools and 482 emissions: 29 losses attributable to
-- the key, 0 to transport. The rate rises with how central the pool is to
-- routing — 7,96 % on SOL-USDC, 5,81 % on NEST-SOL, 3,39 % on AVICI-USDC. The
-- defect therefore hit hardest where the data matters most.
--
-- What the fix buys is COMPLETENESS, not volume: the lost legs weigh 0,04× the
-- average (1 $ against 32 $) — ~8 % of the events for ~0,4 % of the volume.
--
-- `event_index` numbers the transaction's Anchor self-CPIs as
-- `extract_anchor_event_cpis` returns them, INCLUDING those whose discriminator
-- is not (yet) implemented. Numbering only the recognised events would shift
-- every stored index the day one more discriminator is decoded, and a replay
-- would insert duplicates instead of being idempotent. Corollary: the filter in
-- `try_extract_self_cpi_data` is frozen by contract.
--
-- `transaction_index` is nullable and empty. `getTransaction` (Helius) does not
-- return the field — verified live and against the 6 fixtures in the
-- repository. The key reachable today is therefore `(slot, event_index)`, which
-- orders within a transaction and between slots, but not between two
-- transactions of one slot. The column exists so the gRPC/Geyser migration —
-- where the transaction update carries its `index` natively — gives it meaning
-- with no migration and no code change.
--
-- Five tables used to carry their own discriminant in the key (`reward_index`
-- on the four reward tables, `second_position` on split_position). Those
-- columns remain as DATA — they carry business meaning — but left the key,
-- which brings 19 tables down to a single rule instead of three.
--
-- ## Retention is differentiated (009, and 028-035 by inheritance)
--
-- Decision of 15 June 2026, option A: only the high-volume ring-1 streams —
-- swap and liquidity — plus the two claim streams are dropped past 30d, their
-- long-term history living in the continuous aggregates of §13. The punctual /
-- config events and the position-lifecycle events are low in volume but high in
-- semantic value (a pool's config lineage, the open/close/lock history of each
-- position) and are kept indefinitely.
--
-- Compression applies to all 19 regardless: it reclaims space without dropping
-- rows and keeps chunks queryable.
--
-- ⚠️ The compression WARNINGs on `id` / `signature` are expected and harmless;
-- `migrations/README.md` explains why uniqueness is still enforced.
--
-- ## ⚠️ Creation ORDER below is load-bearing
--
-- Postgres truncates auto-generated index names at 63 characters, and two of
-- ours collide after truncation: the unique indexes of
-- `update_reward_duration_events` and `update_reward_funder_events` both reduce
-- to `meteora_damm_v2_update_reward_signature_event_index_timest…`, so the
-- second one created gets an `1` suffix. The tables are therefore declared in
-- the order the original migrations created them (duration before funder),
-- which is what the schema in production has. Naming the indexes explicitly
-- would be cleaner and is a change, not a squash — it does not belong here.
-- ============================================================================


-- ── swap (ring 1, 001) ──────────────────────────────────────────────────────
-- All amount and reserve fields follow the canonical (token_a, token_b) pool
-- ordering. The trader's perspective is recovered by combining
-- `trade_direction` with `amount_a` / `amount_b`.
CREATE TABLE meteora_damm_v2_swap_events (
    id                 BIGSERIAL,
    pool_address       TEXT           NOT NULL,
    signature          TEXT           NOT NULL,

    -- Direction and amounts
    trade_direction    TEXT           NOT NULL,
    amount_a           BIGINT         NOT NULL,
    amount_b           BIGINT         NOT NULL,

    -- Post-swap pool state
    reserve_a_after    BIGINT         NOT NULL,
    reserve_b_after    BIGINT         NOT NULL,
    next_sqrt_price    NUMERIC(39, 0) NOT NULL,

    -- Fee breakdown (DAMM v2 specific). A swap charges its fee in exactly ONE
    -- token, per the pool's collect_fee_mode and the trade direction — which is
    -- what `fee_token_is_a` records.
    claiming_fee       BIGINT         NOT NULL,
    protocol_fee       BIGINT         NOT NULL,
    compounding_fee    BIGINT         NOT NULL,
    referral_fee       BIGINT         NOT NULL,
    fee_token_is_a     BOOLEAN        NOT NULL,

    timestamp          TIMESTAMPTZ    NOT NULL,
    slot               BIGINT         NOT NULL,
    event_index        INTEGER        NOT NULL,
    transaction_index  BIGINT         NULL,
    PRIMARY KEY (id, timestamp),

    CONSTRAINT meteora_damm_v2_swap_events_trade_direction_valid
        CHECK (trade_direction IN ('a_to_b', 'b_to_a'))
);

SELECT create_hypertable('meteora_damm_v2_swap_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');
CREATE INDEX        ON meteora_damm_v2_swap_events (pool_address, timestamp DESC);
CREATE UNIQUE INDEX ON meteora_damm_v2_swap_events (signature, event_index, timestamp);
ALTER TABLE meteora_damm_v2_swap_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_swap_events', INTERVAL '7 days');
SELECT add_retention_policy  ('meteora_damm_v2_swap_events', INTERVAL '30 days');


-- ── liquidity (ring 1, 001) ─────────────────────────────────────────────────
CREATE TABLE meteora_damm_v2_liquidity_events (
    id                   BIGSERIAL,
    pool_address         TEXT           NOT NULL,
    signature            TEXT           NOT NULL,

    liquidity_event_kind TEXT           NOT NULL,
    amount_a             BIGINT         NOT NULL,
    amount_b             BIGINT         NOT NULL,
    liquidity_delta      NUMERIC(39, 0) NOT NULL,

    reserve_a_after      BIGINT         NOT NULL,
    reserve_b_after      BIGINT         NOT NULL,

    position             TEXT           NOT NULL,
    owner                TEXT           NOT NULL,

    timestamp            TIMESTAMPTZ    NOT NULL,
    slot                 BIGINT         NOT NULL,
    event_index          INTEGER        NOT NULL,
    transaction_index    BIGINT         NULL,
    PRIMARY KEY (id, timestamp),

    CONSTRAINT meteora_damm_v2_liquidity_events_kind_valid
        CHECK (liquidity_event_kind IN ('add', 'remove'))
);

SELECT create_hypertable('meteora_damm_v2_liquidity_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');
CREATE INDEX        ON meteora_damm_v2_liquidity_events (pool_address, timestamp DESC);
CREATE INDEX        ON meteora_damm_v2_liquidity_events (position, timestamp DESC);
CREATE UNIQUE INDEX ON meteora_damm_v2_liquidity_events (signature, event_index, timestamp);
ALTER TABLE meteora_damm_v2_liquidity_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_liquidity_events', INTERVAL '7 days');
SELECT add_retention_policy  ('meteora_damm_v2_liquidity_events', INTERVAL '30 days');


-- ── claim_position_fee (ring 1, 001) — an LP claiming their position's fees ──
CREATE TABLE meteora_damm_v2_claim_position_fee_events (
    id                BIGSERIAL,
    pool_address      TEXT        NOT NULL,
    signature         TEXT        NOT NULL,

    position          TEXT        NOT NULL,
    owner             TEXT        NOT NULL,

    fee_a_claimed     BIGINT      NOT NULL,
    fee_b_claimed     BIGINT      NOT NULL,

    timestamp         TIMESTAMPTZ NOT NULL,
    slot              BIGINT      NOT NULL,
    event_index       INTEGER     NOT NULL,
    transaction_index BIGINT      NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_claim_position_fee_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');
CREATE INDEX        ON meteora_damm_v2_claim_position_fee_events (pool_address, timestamp DESC);
CREATE INDEX        ON meteora_damm_v2_claim_position_fee_events (position, timestamp DESC);
CREATE UNIQUE INDEX ON meteora_damm_v2_claim_position_fee_events (signature, event_index, timestamp);
ALTER TABLE meteora_damm_v2_claim_position_fee_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_claim_position_fee_events', INTERVAL '7 days');
SELECT add_retention_policy  ('meteora_damm_v2_claim_position_fee_events', INTERVAL '30 days');


-- ── claim_reward (ring 1, 001) — an LP claiming farming rewards ─────────────
CREATE TABLE meteora_damm_v2_claim_reward_events (
    id                BIGSERIAL,
    pool_address      TEXT        NOT NULL,
    signature         TEXT        NOT NULL,

    position          TEXT        NOT NULL,
    owner             TEXT        NOT NULL,

    mint_reward       TEXT        NOT NULL,
    reward_index      SMALLINT    NOT NULL,
    total_reward      BIGINT      NOT NULL,

    timestamp         TIMESTAMPTZ NOT NULL,
    slot              BIGINT      NOT NULL,
    event_index       INTEGER     NOT NULL,
    transaction_index BIGINT      NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_claim_reward_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');
CREATE INDEX        ON meteora_damm_v2_claim_reward_events (pool_address, timestamp DESC);
CREATE INDEX        ON meteora_damm_v2_claim_reward_events (position, timestamp DESC);
CREATE UNIQUE INDEX ON meteora_damm_v2_claim_reward_events (signature, event_index, timestamp);
ALTER TABLE meteora_damm_v2_claim_reward_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_claim_reward_events', INTERVAL '7 days');
SELECT add_retention_policy  ('meteora_damm_v2_claim_reward_events', INTERVAL '30 days');


-- ── create_position (ring 2, 002) ───────────────────────────────────────────
-- Emitted when an LP opens a new, EMPTY position. The position is NFT-backed
-- (`position_nft_mint`); `position` is the PDA holding its state. No token
-- amounts and no reserves — liquidity arrives later through a liquidity event.
CREATE TABLE meteora_damm_v2_create_position_events (
    id                BIGSERIAL,
    pool_address      TEXT        NOT NULL,
    signature         TEXT        NOT NULL,

    owner             TEXT        NOT NULL,
    position          TEXT        NOT NULL,
    position_nft_mint TEXT        NOT NULL,

    timestamp         TIMESTAMPTZ NOT NULL,
    slot              BIGINT      NOT NULL,
    event_index       INTEGER     NOT NULL,
    transaction_index BIGINT      NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_create_position_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');
CREATE INDEX        ON meteora_damm_v2_create_position_events (pool_address, timestamp DESC);
CREATE INDEX        ON meteora_damm_v2_create_position_events (position, timestamp DESC);
CREATE UNIQUE INDEX ON meteora_damm_v2_create_position_events (signature, event_index, timestamp);
ALTER TABLE meteora_damm_v2_create_position_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_create_position_events', INTERVAL '7 days');
-- No retention: position lifecycle is kept forever (009).


-- ── close_position (ring 2, 003) ────────────────────────────────────────────
-- Paired with create_position, the two delimit a position's lifespan.
CREATE TABLE meteora_damm_v2_close_position_events (
    id                BIGSERIAL,
    pool_address      TEXT        NOT NULL,
    signature         TEXT        NOT NULL,

    owner             TEXT        NOT NULL,
    position          TEXT        NOT NULL,
    position_nft_mint TEXT        NOT NULL,

    timestamp         TIMESTAMPTZ NOT NULL,
    slot              BIGINT      NOT NULL,
    event_index       INTEGER     NOT NULL,
    transaction_index BIGINT      NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_close_position_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');
CREATE INDEX        ON meteora_damm_v2_close_position_events (pool_address, timestamp DESC);
CREATE INDEX        ON meteora_damm_v2_close_position_events (position, timestamp DESC);
CREATE UNIQUE INDEX ON meteora_damm_v2_close_position_events (signature, event_index, timestamp);
ALTER TABLE meteora_damm_v2_close_position_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_close_position_events', INTERVAL '7 days');


-- ── lock_position (ring 2, 004) ─────────────────────────────────────────────
-- An LP locking a position under a vesting schedule: cliff_unlock_liquidity
-- unlocks at cliff_point, then liquidity_per_period every period_frequency for
-- number_of_period periods.
CREATE TABLE meteora_damm_v2_lock_position_events (
    id                     BIGSERIAL,
    pool_address           TEXT           NOT NULL,
    signature              TEXT           NOT NULL,

    position               TEXT           NOT NULL,
    owner                  TEXT           NOT NULL,
    vesting                TEXT           NOT NULL,

    cliff_point            BIGINT         NOT NULL,
    period_frequency       BIGINT         NOT NULL,
    cliff_unlock_liquidity NUMERIC(39, 0) NOT NULL,
    liquidity_per_period   NUMERIC(39, 0) NOT NULL,
    number_of_period       INTEGER        NOT NULL,   -- u16, so INTEGER

    timestamp              TIMESTAMPTZ    NOT NULL,
    slot                   BIGINT         NOT NULL,
    event_index            INTEGER        NOT NULL,
    transaction_index      BIGINT         NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_lock_position_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');
CREATE INDEX        ON meteora_damm_v2_lock_position_events (pool_address, timestamp DESC);
CREATE INDEX        ON meteora_damm_v2_lock_position_events (position, timestamp DESC);
CREATE UNIQUE INDEX ON meteora_damm_v2_lock_position_events (signature, event_index, timestamp);
ALTER TABLE meteora_damm_v2_lock_position_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_lock_position_events', INTERVAL '7 days');


-- ── permanent_lock_position (ring 2, 005) ───────────────────────────────────
-- Part of a position's liquidity locked permanently (no vesting, never
-- unlocks). lock_liquidity_amount is what this action locked;
-- total_permanent_locked_liquidity is the position's running total afterwards.
-- No owner column — the on-chain event only carries pool and position.
CREATE TABLE meteora_damm_v2_permanent_lock_position_events (
    id                               BIGSERIAL,
    pool_address                     TEXT           NOT NULL,
    signature                        TEXT           NOT NULL,

    position                         TEXT           NOT NULL,
    lock_liquidity_amount            NUMERIC(39, 0) NOT NULL,
    total_permanent_locked_liquidity NUMERIC(39, 0) NOT NULL,

    timestamp                        TIMESTAMPTZ    NOT NULL,
    slot                             BIGINT         NOT NULL,
    event_index                      INTEGER        NOT NULL,
    transaction_index                BIGINT         NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_permanent_lock_position_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');
CREATE INDEX        ON meteora_damm_v2_permanent_lock_position_events (pool_address, timestamp DESC);
CREATE INDEX        ON meteora_damm_v2_permanent_lock_position_events (position, timestamp DESC);
CREATE UNIQUE INDEX ON meteora_damm_v2_permanent_lock_position_events (signature, event_index, timestamp);
ALTER TABLE meteora_damm_v2_permanent_lock_position_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_permanent_lock_position_events', INTERVAL '7 days');


-- ── initialize_pool (ring 2, 006) — pool genesis ────────────────────────────
-- Emitted once when a DAMM v2 pool is created. Carries both mints, the initial
-- AMM state (sqrt price + bounds, seeded liquidity), the activation schedule,
-- and the seeded token amounts.
--
-- "voie C": the fee configuration is captured UNDECODED as a raw borsh blob
-- (pool_fees_raw). `pools.fee_bps` (§2) and the fee shape (§8) are derived from
-- these stored bytes.
--
-- activation_point is a raw unix timestamp in seconds, not a TIMESTAMPTZ.
CREATE TABLE meteora_damm_v2_initialize_pool_events (
    id                BIGSERIAL,
    pool_address      TEXT           NOT NULL,
    signature         TEXT           NOT NULL,

    token_a_mint      TEXT           NOT NULL,
    token_b_mint      TEXT           NOT NULL,
    creator           TEXT           NOT NULL,
    payer             TEXT           NOT NULL,
    alpha_vault       TEXT           NOT NULL,

    sqrt_min_price    NUMERIC(39, 0) NOT NULL,
    sqrt_max_price    NUMERIC(39, 0) NOT NULL,
    sqrt_price        NUMERIC(39, 0) NOT NULL,
    liquidity         NUMERIC(39, 0) NOT NULL,

    activation_type   SMALLINT       NOT NULL,
    activation_point  BIGINT         NOT NULL,
    collect_fee_mode  SMALLINT       NOT NULL,
    pool_type         SMALLINT       NOT NULL,

    token_a_flag      SMALLINT       NOT NULL,
    token_b_flag      SMALLINT       NOT NULL,
    token_a_amount    BIGINT         NOT NULL,
    token_b_amount    BIGINT         NOT NULL,
    total_amount_a    BIGINT         NOT NULL,
    total_amount_b    BIGINT         NOT NULL,

    pool_fees_raw     BYTEA          NOT NULL,

    timestamp         TIMESTAMPTZ    NOT NULL,
    slot              BIGINT         NOT NULL,
    event_index       INTEGER        NOT NULL,
    transaction_index BIGINT         NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_initialize_pool_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');
CREATE INDEX        ON meteora_damm_v2_initialize_pool_events (pool_address, timestamp DESC);
CREATE UNIQUE INDEX ON meteora_damm_v2_initialize_pool_events (signature, event_index, timestamp);
ALTER TABLE meteora_damm_v2_initialize_pool_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_initialize_pool_events', INTERVAL '7 days');


-- ── set_pool_status (ring 2, 007) ───────────────────────────────────────────
-- `status` is the raw on-chain byte, stored uninterpreted (u8 → SMALLINT).
CREATE TABLE meteora_damm_v2_set_pool_status_events (
    id                BIGSERIAL,
    pool_address      TEXT        NOT NULL,
    signature         TEXT        NOT NULL,

    status            SMALLINT    NOT NULL,

    timestamp         TIMESTAMPTZ NOT NULL,
    slot              BIGINT      NOT NULL,
    event_index       INTEGER     NOT NULL,
    transaction_index BIGINT      NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_set_pool_status_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');
CREATE INDEX        ON meteora_damm_v2_set_pool_status_events (pool_address, timestamp DESC);
CREATE UNIQUE INDEX ON meteora_damm_v2_set_pool_status_events (signature, event_index, timestamp);
ALTER TABLE meteora_damm_v2_set_pool_status_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_set_pool_status_events', INTERVAL '7 days');


-- ── update_pool_fees (ring 2, 008) ──────────────────────────────────────────
-- An operator updating a pool's fee parameters. "voie C": the new parameters
-- are stored as a raw, undecoded borsh blob (params_raw) — the trailing
-- UpdatePoolFeesParameters of the wire event, captured verbatim. The indexer
-- raises `pools.needs_refresh` (§2) instead of decoding the delta; yog-context
-- then re-reads the account, which carries resolved state rather than a delta.
CREATE TABLE meteora_damm_v2_update_pool_fees_events (
    id                BIGSERIAL,
    pool_address      TEXT        NOT NULL,
    signature         TEXT        NOT NULL,

    operator          TEXT        NOT NULL,
    params_raw        BYTEA       NOT NULL,

    timestamp         TIMESTAMPTZ NOT NULL,
    slot              BIGINT      NOT NULL,
    event_index       INTEGER     NOT NULL,
    transaction_index BIGINT      NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_update_pool_fees_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');
CREATE INDEX        ON meteora_damm_v2_update_pool_fees_events (pool_address, timestamp DESC);
CREATE UNIQUE INDEX ON meteora_damm_v2_update_pool_fees_events (signature, event_index, timestamp);
ALTER TABLE meteora_damm_v2_update_pool_fees_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_update_pool_fees_events', INTERVAL '7 days');


-- ── claim_protocol_fee (028) ────────────────────────────────────────────────
-- The protocol operator withdrawing Meteora's accrued *protocol* share of a
-- pool's trading fees — distinct from claim_position_fee (an LP claiming their
-- own position's fees). Decoded from the `emit_cpi!` EvtClaimProtocolFee; the
-- differently-shaped EvtClaimProtocolFee2 is a plain `emit!` log, not an
-- event_cpi, and is not captured by the indexer.
--
-- token_a_amount / token_b_amount: absolute amounts withdrawn this claim,
-- canonical pool ordering.
CREATE TABLE meteora_damm_v2_claim_protocol_fee_events (
    id                BIGSERIAL,
    pool_address      TEXT        NOT NULL,
    signature         TEXT        NOT NULL,

    token_a_amount    BIGINT      NOT NULL,
    token_b_amount    BIGINT      NOT NULL,

    timestamp         TIMESTAMPTZ NOT NULL,
    slot              BIGINT      NOT NULL,
    event_index       INTEGER     NOT NULL,
    transaction_index BIGINT      NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_claim_protocol_fee_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');
CREATE INDEX        ON meteora_damm_v2_claim_protocol_fee_events (pool_address, timestamp DESC);
CREATE UNIQUE INDEX ON meteora_damm_v2_claim_protocol_fee_events (signature, event_index, timestamp);
ALTER TABLE meteora_damm_v2_claim_protocol_fee_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_claim_protocol_fee_events', INTERVAL '7 days');


-- ── initialize_reward (029) — a farm reward slot opened ─────────────────────
-- A pool carries a fixed number of slots addressed by reward_index (cp-amm's
-- NUM_REWARDS = 2); each streams one reward_mint token to in-range LPs at a
-- constant rate. Opening a slot distributes nothing on its own: the tokens and
-- the emission rate arrive with fund_reward (030), usually in the same
-- transaction. This row is the "a new farm launched" marker.
--
-- reward_duration: the length of a funding window in SECONDS (e.g. 604800 =
-- 7 days), not a timestamp.
CREATE TABLE meteora_damm_v2_initialize_reward_events (
    id                BIGSERIAL,
    pool_address      TEXT        NOT NULL,
    signature         TEXT        NOT NULL,

    reward_mint       TEXT        NOT NULL,
    funder            TEXT        NOT NULL,
    creator           TEXT        NOT NULL,
    reward_index      SMALLINT    NOT NULL,
    reward_duration   BIGINT      NOT NULL,

    timestamp         TIMESTAMPTZ NOT NULL,
    slot              BIGINT      NOT NULL,
    event_index       INTEGER     NOT NULL,
    transaction_index BIGINT      NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_initialize_reward_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');
CREATE INDEX        ON meteora_damm_v2_initialize_reward_events (pool_address, timestamp DESC);
CREATE UNIQUE INDEX ON meteora_damm_v2_initialize_reward_events (signature, event_index, timestamp);
ALTER TABLE meteora_damm_v2_initialize_reward_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_initialize_reward_events', INTERVAL '7 days');


-- ── fund_reward (030) — farm slot funded, emission rate set ─────────────────
-- A funder depositing reward tokens into a farm slot, which makes the program
-- recompute the slot's emission rate.
--
-- ⚠️ pre_reward_rate / post_reward_rate are Q64.64 — NOT plain rates. They are
-- reward base units per second in Q64.64 fixed point: DIVIDE BY 2^64
-- (18446744073709551616) to read them as a rate. On a freshly opened slot,
-- verified against real on-chain data:
--
--     post_reward_rate = (amount << 64) / reward_duration
--
-- Reading these columns without the shift overstates the emission rate by 19
-- orders of magnitude. reward_duration lives on the initialize_reward row for
-- the same (pool, reward_index).
--
-- Carry-forward has no column, by design: funding a slot that is still running
-- folds the undistributed remainder of the current window into the new one, so
-- post_reward_rate reflects amount + leftover. cp-amm exposes this only through
-- the rate pair. Recover it as:
--
--     (post_reward_rate * reward_duration >> 64) - amount
--
-- `amount` is what the funder sent; transfer_fee_excluded_amount_in is what
-- landed in the vault (they differ only for Token-2022 mints with a transfer
-- fee). reward_duration_end is a raw unix timestamp in seconds.
CREATE TABLE meteora_damm_v2_fund_reward_events (
    id                              BIGSERIAL,
    pool_address                    TEXT           NOT NULL,
    signature                       TEXT           NOT NULL,

    funder                          TEXT           NOT NULL,
    mint_reward                     TEXT           NOT NULL,
    reward_index                    SMALLINT       NOT NULL,
    amount                          BIGINT         NOT NULL,
    transfer_fee_excluded_amount_in BIGINT         NOT NULL,
    reward_duration_end             BIGINT         NOT NULL,
    pre_reward_rate                 NUMERIC(39, 0) NOT NULL,
    post_reward_rate                NUMERIC(39, 0) NOT NULL,

    timestamp                       TIMESTAMPTZ    NOT NULL,
    slot                            BIGINT         NOT NULL,
    event_index                     INTEGER        NOT NULL,
    transaction_index               BIGINT         NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_fund_reward_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');
CREATE INDEX        ON meteora_damm_v2_fund_reward_events (pool_address, timestamp DESC);
CREATE UNIQUE INDEX ON meteora_damm_v2_fund_reward_events (signature, event_index, timestamp);
ALTER TABLE meteora_damm_v2_fund_reward_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_fund_reward_events', INTERVAL '7 days');


-- ── withdraw_ineligible_reward (031) ────────────────────────────────────────
-- A funder reclaiming reward tokens that nobody could earn. Rewards accrue
-- continuously once a slot is funded, but only in-range LPs are eligible;
-- whatever accrued while the pool held ZERO eligible liquidity can never be
-- claimed by anyone. This returns it to the funder, and is only permitted after
-- the emission window has ended.
--
-- `amount` is legitimately ZERO when the pool always had eligible liquidity:
-- the instruction still runs and still emits (the captured fixture is exactly
-- that case). No reward_index on this event — cp-amm identifies the slot by
-- reward_mint here.
CREATE TABLE meteora_damm_v2_withdraw_ineligible_reward_events (
    id                BIGSERIAL,
    pool_address      TEXT        NOT NULL,
    signature         TEXT        NOT NULL,

    reward_mint       TEXT        NOT NULL,
    amount            BIGINT      NOT NULL,

    timestamp         TIMESTAMPTZ NOT NULL,
    slot              BIGINT      NOT NULL,
    event_index       INTEGER     NOT NULL,
    transaction_index BIGINT      NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_withdraw_ineligible_reward_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');
CREATE INDEX        ON meteora_damm_v2_withdraw_ineligible_reward_events (pool_address, timestamp DESC);
CREATE UNIQUE INDEX ON meteora_damm_v2_withdraw_ineligible_reward_events (signature, event_index, timestamp);
ALTER TABLE meteora_damm_v2_withdraw_ineligible_reward_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_withdraw_ineligible_reward_events', INTERVAL '7 days');


-- ── update_reward_duration (032) — farm slot re-paced ───────────────────────
-- An admin changing the length of a farm slot's funding window. This changes
-- the emission rate every SUBSEQUENT funding will compute (rate = amount /
-- duration, see 030) — it does not re-rate the window already running. A
-- duration stretched without fresh funding dilutes the farm: the same tokens
-- spread thinner, lower yield per LP.
--
-- Durations are in SECONDS, not timestamps.
--
-- NO ON-CHAIN FIXTURE for this event: the layout comes from the cp-amm source
-- alone (single emit_cpi! site, verified). Guarded in core by a field-mapping
-- test and a byte-level layout-pinning test rather than a real transaction.
--
-- ⚠️ Declared BEFORE update_reward_funder — see the ordering note in the §12
-- header.
CREATE TABLE meteora_damm_v2_update_reward_duration_events (
    id                  BIGSERIAL,
    pool_address        TEXT        NOT NULL,
    signature           TEXT        NOT NULL,

    reward_index        SMALLINT    NOT NULL,
    old_reward_duration BIGINT      NOT NULL,
    new_reward_duration BIGINT      NOT NULL,

    timestamp           TIMESTAMPTZ NOT NULL,
    slot                BIGINT      NOT NULL,
    event_index         INTEGER     NOT NULL,
    transaction_index   BIGINT      NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_update_reward_duration_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');
CREATE INDEX        ON meteora_damm_v2_update_reward_duration_events (pool_address, timestamp DESC);
CREATE UNIQUE INDEX ON meteora_damm_v2_update_reward_duration_events (signature, event_index, timestamp);
ALTER TABLE meteora_damm_v2_update_reward_duration_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_update_reward_duration_events', INTERVAL '7 days');


-- ── update_reward_funder (033) — farm funding right moved ───────────────────
-- An admin transferring the right to fund a farm slot from one wallet to
-- another. Moves no tokens and does not touch the emission rate: it only
-- changes which wallet may call fund_reward on this reward_index, and which
-- wallet receives reclaimed rewards. Read as provenance — who is paying for the
-- incentive, and when the farm changed hands.
--
-- NO ON-CHAIN FIXTURE, same as 032.
CREATE TABLE meteora_damm_v2_update_reward_funder_events (
    id                BIGSERIAL,
    pool_address      TEXT        NOT NULL,
    signature         TEXT        NOT NULL,

    reward_index      SMALLINT    NOT NULL,
    old_funder        TEXT        NOT NULL,
    new_funder        TEXT        NOT NULL,

    timestamp         TIMESTAMPTZ NOT NULL,
    slot              BIGINT      NOT NULL,
    event_index       INTEGER     NOT NULL,
    transaction_index BIGINT      NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_update_reward_funder_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');
CREATE INDEX        ON meteora_damm_v2_update_reward_funder_events (pool_address, timestamp DESC);
CREATE UNIQUE INDEX ON meteora_damm_v2_update_reward_funder_events (signature, event_index, timestamp);
ALTER TABLE meteora_damm_v2_update_reward_funder_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_update_reward_funder_events', INTERVAL '7 days');


-- ── withdraw_dead_liquidity_reward (034) ────────────────────────────────────
-- A funder reclaiming the reward share that accrued to DEAD LIQUIDITY —
-- liquidity permanently locked with no owner left to claim against it.
--
-- Distinct from 031 despite an identical shape: same three columns, same
-- 72-byte wire payload, only the Anchor discriminator tells them apart. They
-- stay separate tables because they record different on-chain facts (one
-- on-chain event → one table), and because their emission semantics differ —
-- 031 emits UNCONDITIONALLY (amount = 0 rows are legitimate), while this one
-- emits only inside `if dead_liquidity_reward > 0`, so `amount` is ALWAYS > 0
-- here. A zero row would mean our decoding drifted.
--
-- NO ON-CHAIN FIXTURE, same as 032/033.
CREATE TABLE meteora_damm_v2_withdraw_dead_liquidity_reward_events (
    id                BIGSERIAL,
    pool_address      TEXT        NOT NULL,
    signature         TEXT        NOT NULL,

    reward_mint       TEXT        NOT NULL,
    amount            BIGINT      NOT NULL,

    timestamp         TIMESTAMPTZ NOT NULL,
    slot              BIGINT      NOT NULL,
    event_index       INTEGER     NOT NULL,
    transaction_index BIGINT      NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_withdraw_dead_liquidity_reward_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');
CREATE INDEX        ON meteora_damm_v2_withdraw_dead_liquidity_reward_events (pool_address, timestamp DESC);
CREATE UNIQUE INDEX ON meteora_damm_v2_withdraw_dead_liquidity_reward_events (signature, event_index, timestamp);
ALTER TABLE meteora_damm_v2_withdraw_dead_liquidity_reward_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_withdraw_dead_liquidity_reward_events', INTERVAL '7 days');


-- ── split_position (035) ────────────────────────────────────────────────────
-- A position transfers a FRACTION of its contents into a second position, which
-- may belong to a different owner. Each component is split independently:
-- unlocked liquidity, permanently locked liquidity, vesting liquidity, pending
-- fees A/B, pending farm rewards 0/1.
--
-- Product angle: a split moves liquidity BETWEEN TWO WALLETS and leaves a
-- traceable event — unlike transferring the position NFT outright, which is the
-- blind spot of any LP-concentration score.
--
-- Source event: EvtSplitPosition3 only. cp-amm has two instructions
-- (`split_position`, `split_position2`) routing to one handler, which emits
-- BOTH EvtSplitPosition2 (deprecated since 0.1.8) and EvtSplitPosition3
-- unconditionally on every split. They describe the SAME split; v3 is a strict
-- superset. The indexer recognises the v2 discriminator and drops it
-- deliberately — indexing both would double-count every split.
--
-- Column groups:
--   split_*   what actually MOVED from the first position to the second
--   first_*   state of the first position AFTER the split
--   second_*  state of the second position AFTER the split
--   num_*     the fractions REQUESTED, numerators over 1e9
--             (SPLIT_POSITION_DENOMINATOR). Kept alongside the realised amounts
--             because the gap between the two is itself informative: rounding,
--             or a component that had nothing to give. u32 → BIGINT (lossless).
CREATE TABLE meteora_damm_v2_split_position_events (
    id                               BIGSERIAL,
    pool_address                     TEXT           NOT NULL,
    signature                        TEXT           NOT NULL,

    first_owner                      TEXT           NOT NULL,
    second_owner                     TEXT           NOT NULL,
    first_position                   TEXT           NOT NULL,
    second_position                  TEXT           NOT NULL,
    current_sqrt_price               NUMERIC(39, 0) NOT NULL,

    -- What moved
    split_permanent_locked_liquidity NUMERIC(39, 0) NOT NULL,
    split_unlocked_liquidity         NUMERIC(39, 0) NOT NULL,
    split_vested_liquidity           NUMERIC(39, 0) NOT NULL,
    split_fee_a                      BIGINT         NOT NULL,
    split_fee_b                      BIGINT         NOT NULL,
    split_reward_0                   BIGINT         NOT NULL,
    split_reward_1                   BIGINT         NOT NULL,

    -- First position after the split
    first_unlocked_liquidity         NUMERIC(39, 0) NOT NULL,
    first_permanent_locked_liquidity NUMERIC(39, 0) NOT NULL,
    first_vested_liquidity           NUMERIC(39, 0) NOT NULL,
    first_fee_a                      BIGINT         NOT NULL,
    first_fee_b                      BIGINT         NOT NULL,
    first_reward_0                   BIGINT         NOT NULL,
    first_reward_1                   BIGINT         NOT NULL,

    -- Second position after the split
    second_unlocked_liquidity         NUMERIC(39, 0) NOT NULL,
    second_permanent_locked_liquidity NUMERIC(39, 0) NOT NULL,
    second_vested_liquidity           NUMERIC(39, 0) NOT NULL,
    second_fee_a                      BIGINT         NOT NULL,
    second_fee_b                      BIGINT         NOT NULL,
    second_reward_0                   BIGINT         NOT NULL,
    second_reward_1                   BIGINT         NOT NULL,

    -- Requested fractions (numerators over 1e9)
    num_unlocked_liquidity           BIGINT         NOT NULL,
    num_permanent_locked_liquidity   BIGINT         NOT NULL,
    num_fee_a                        BIGINT         NOT NULL,
    num_fee_b                        BIGINT         NOT NULL,
    num_reward_0                     BIGINT         NOT NULL,
    num_reward_1                     BIGINT         NOT NULL,
    num_inner_vesting_liquidity      BIGINT         NOT NULL,

    timestamp                        TIMESTAMPTZ    NOT NULL,
    slot                             BIGINT         NOT NULL,
    event_index                      INTEGER        NOT NULL,
    transaction_index                BIGINT         NULL,
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('meteora_damm_v2_split_position_events',
    'timestamp', chunk_time_interval => INTERVAL '7 days');
CREATE INDEX        ON meteora_damm_v2_split_position_events (pool_address, timestamp DESC);
-- Both wallets are access paths: a split is the one traceable way liquidity
-- moves between owners, so concentration analytics query from either side.
CREATE INDEX        ON meteora_damm_v2_split_position_events (first_owner, timestamp DESC);
CREATE INDEX        ON meteora_damm_v2_split_position_events (second_owner, timestamp DESC);
CREATE UNIQUE INDEX ON meteora_damm_v2_split_position_events (signature, event_index, timestamp);
ALTER TABLE meteora_damm_v2_split_position_events SET (
    timescaledb.compress,
    timescaledb.compress_orderby   = 'timestamp DESC',
    timescaledb.compress_segmentby = 'pool_address'
);
SELECT add_compression_policy('meteora_damm_v2_split_position_events', INTERVAL '7 days');


-- ── Grants on the 19 event tables ───────────────────────────────────────────
-- Table-level, so a column added by a later migration is covered the moment it
-- exists. SELECT for yog_api/yog_context/yog_signals and sequence USAGE for
-- yog_indexer come from the default privileges in `setup_roles.sql`.
GRANT SELECT, INSERT, UPDATE
    ON meteora_damm_v2_swap_events,
       meteora_damm_v2_liquidity_events,
       meteora_damm_v2_claim_position_fee_events,
       meteora_damm_v2_claim_reward_events,
       meteora_damm_v2_create_position_events,
       meteora_damm_v2_close_position_events,
       meteora_damm_v2_lock_position_events,
       meteora_damm_v2_permanent_lock_position_events,
       meteora_damm_v2_initialize_pool_events,
       meteora_damm_v2_set_pool_status_events,
       meteora_damm_v2_update_pool_fees_events,
       meteora_damm_v2_claim_protocol_fee_events,
       meteora_damm_v2_initialize_reward_events,
       meteora_damm_v2_fund_reward_events,
       meteora_damm_v2_withdraw_ineligible_reward_events,
       meteora_damm_v2_update_reward_duration_events,
       meteora_damm_v2_update_reward_funder_events,
       meteora_damm_v2_withdraw_dead_liquidity_reward_events,
       meteora_damm_v2_split_position_events
    TO yog_indexer;

GRANT SELECT
    ON meteora_damm_v2_swap_events,
       meteora_damm_v2_liquidity_events,
       meteora_damm_v2_claim_position_fee_events,
       meteora_damm_v2_claim_reward_events,
       meteora_damm_v2_create_position_events,
       meteora_damm_v2_close_position_events,
       meteora_damm_v2_lock_position_events,
       meteora_damm_v2_permanent_lock_position_events,
       meteora_damm_v2_initialize_pool_events,
       meteora_damm_v2_set_pool_status_events,
       meteora_damm_v2_update_pool_fees_events,
       meteora_damm_v2_claim_protocol_fee_events,
       meteora_damm_v2_initialize_reward_events,
       meteora_damm_v2_fund_reward_events,
       meteora_damm_v2_withdraw_ineligible_reward_events,
       meteora_damm_v2_update_reward_duration_events,
       meteora_damm_v2_update_reward_funder_events,
       meteora_damm_v2_withdraw_dead_liquidity_reward_events,
       meteora_damm_v2_split_position_events
    TO yog_api;

-- Sequences behind BIGSERIAL columns.
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO yog_indexer;


-- ============================================================================
-- §13 — the four hourly continuous aggregates  [010-013, reshaped by 014, 017]
-- ============================================================================
-- Durable rollups. Two roles: history — they survive the 30d retention drop on
-- the raw hypertables, holding hourly aggregates per pool indefinitely — and
-- perf, feeding the read paths without scanning raw rows.
--
-- USD is NOT stored here: a continuous aggregate cannot join token_prices. We
-- store RAW token amounts and value them at read time, per bucket, at the price
-- as-of that bucket — preserving trade-time valuation (the price when it
-- happened, not the current price). The mints come from `pools` at read time,
-- which is why they are not carried in the aggregates (014 removed them from
-- the raw event tables along with the bug they carried).
--
-- `materialized_only = false` → real-time aggregation: reads union the
-- materialized buckets with a live query over the not-yet-materialized recent
-- raw rows, so the current partial hour is always reflected.
--
-- `WITH NO DATA` + a refresh policy, never `refresh_continuous_aggregate`,
-- which cannot run in a transaction — the statement has to stay valid inside
-- the sqlx migration transaction; the policy backfills and keeps it current.
-- start_offset spans the full 30d retention window (raw rows never live
-- longer), end_offset leaves the current hour to real-time aggregation.


-- ── swap volume + realized fees (010, rebuilt by 014 and 017) ───────────────
-- Direction-filtered sums: the valuation counts only the INPUT side of each
-- swap (a_to_b → amount_a, b_to_a → amount_b), matching the read-time rule.
--
-- A swap charges its fee in exactly ONE token (A or B), per the pool's
-- collect_fee_mode and the trade direction — captured by fee_token_is_a. Fees
-- are therefore summed split on that flag, and protocol_fee separately, so the
-- LP share is (fee_in_x - protocol_fee_in_x) and the effective rate is
-- fee_in_x / volume_in_x within the same bucket and token.
CREATE MATERIALIZED VIEW meteora_damm_v2_swap_events_hourly
WITH (timescaledb.continuous, timescaledb.materialized_only = false) AS
SELECT
    time_bucket('1 hour', timestamp)                        AS bucket,
    pool_address,
    SUM(amount_a) FILTER (WHERE trade_direction = 'a_to_b') AS volume_in_a,
    SUM(amount_b) FILTER (WHERE trade_direction = 'b_to_a') AS volume_in_b,
    COUNT(*)                                                AS swap_count,
    SUM(claiming_fee + protocol_fee + compounding_fee + referral_fee)
        FILTER (WHERE fee_token_is_a)                       AS fee_in_a,
    SUM(claiming_fee + protocol_fee + compounding_fee + referral_fee)
        FILTER (WHERE NOT fee_token_is_a)                   AS fee_in_b,
    SUM(protocol_fee) FILTER (WHERE fee_token_is_a)         AS protocol_fee_in_a,
    SUM(protocol_fee) FILTER (WHERE NOT fee_token_is_a)     AS protocol_fee_in_b
FROM meteora_damm_v2_swap_events
GROUP BY bucket, pool_address
WITH NO DATA;

SELECT add_continuous_aggregate_policy('meteora_damm_v2_swap_events_hourly',
    start_offset      => INTERVAL '31 days',
    end_offset        => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');

GRANT SELECT ON meteora_damm_v2_swap_events_hourly TO yog_api;


-- ── liquidity add/remove (011, rebuilt by 014) ──────────────────────────────
-- Liquidity events carry a direction in `liquidity_event_kind ∈ ('add',
-- 'remove')` and `liquidity_delta` is an unsigned magnitude (u128) — summing it
-- across both kinds would be meaningless, so every value is split by kind.
CREATE MATERIALIZED VIEW meteora_damm_v2_liquidity_events_hourly
WITH (timescaledb.continuous, timescaledb.materialized_only = false) AS
SELECT
    time_bucket('1 hour', timestamp)                                    AS bucket,
    pool_address,
    SUM(amount_a)        FILTER (WHERE liquidity_event_kind = 'add')    AS amount_a_added,
    SUM(amount_b)        FILTER (WHERE liquidity_event_kind = 'add')    AS amount_b_added,
    SUM(amount_a)        FILTER (WHERE liquidity_event_kind = 'remove') AS amount_a_removed,
    SUM(amount_b)        FILTER (WHERE liquidity_event_kind = 'remove') AS amount_b_removed,
    SUM(liquidity_delta) FILTER (WHERE liquidity_event_kind = 'add')    AS liquidity_added,
    SUM(liquidity_delta) FILTER (WHERE liquidity_event_kind = 'remove') AS liquidity_removed,
    COUNT(*)             FILTER (WHERE liquidity_event_kind = 'add')    AS add_count,
    COUNT(*)             FILTER (WHERE liquidity_event_kind = 'remove') AS remove_count
FROM meteora_damm_v2_liquidity_events
GROUP BY bucket, pool_address
WITH NO DATA;

SELECT add_continuous_aggregate_policy('meteora_damm_v2_liquidity_events_hourly',
    start_offset      => INTERVAL '31 days',
    end_offset        => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');

GRANT SELECT ON meteora_damm_v2_liquidity_events_hourly TO yog_api;


-- ── position-fee claims (012) ───────────────────────────────────────────────
-- No direction to split (a claim is a claim) and the source table carries no
-- token mints — only fee_a_claimed / fee_b_claimed in raw units.
CREATE MATERIALIZED VIEW meteora_damm_v2_claim_position_fee_events_hourly
WITH (timescaledb.continuous, timescaledb.materialized_only = false) AS
SELECT
    time_bucket('1 hour', timestamp) AS bucket,
    pool_address,
    SUM(fee_a_claimed)               AS fee_a_claimed,
    SUM(fee_b_claimed)               AS fee_b_claimed,
    COUNT(*)                         AS claim_count
FROM meteora_damm_v2_claim_position_fee_events
GROUP BY bucket, pool_address
WITH NO DATA;

SELECT add_continuous_aggregate_policy('meteora_damm_v2_claim_position_fee_events_hourly',
    start_offset      => INTERVAL '31 days',
    end_offset        => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');

GRANT SELECT ON meteora_damm_v2_claim_position_fee_events_hourly TO yog_api;


-- ── reward claims (013) ─────────────────────────────────────────────────────
-- A pool can emit rewards in several tokens, so the rollup is grouped by
-- mint_reward in addition to the bucket — summing total_reward across distinct
-- reward tokens would be meaningless.
CREATE MATERIALIZED VIEW meteora_damm_v2_claim_reward_events_hourly
WITH (timescaledb.continuous, timescaledb.materialized_only = false) AS
SELECT
    time_bucket('1 hour', timestamp) AS bucket,
    pool_address,
    mint_reward,
    SUM(total_reward)                AS total_reward,
    COUNT(*)                         AS claim_count
FROM meteora_damm_v2_claim_reward_events
GROUP BY bucket, pool_address, mint_reward
WITH NO DATA;

SELECT add_continuous_aggregate_policy('meteora_damm_v2_claim_reward_events_hourly',
    start_offset      => INTERVAL '31 days',
    end_offset        => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');

GRANT SELECT ON meteora_damm_v2_claim_reward_events_hourly TO yog_api;


-- ============================================================================
-- §14 — cross-protocol VIEWs: the unified read surface  [001, 014]
-- ============================================================================
-- These expose the slim common columns across protocols, with a `protocol` text
-- column injected per UNION ALL branch. Today there is only one underlying
-- table per VIEW; future protocols add UNION ALL branches without touching the
-- API code.
--
-- Protocol-specific columns (next_sqrt_price, the fee breakdown, and the
-- position-in-chain triple of §12) are deliberately NOT here — their contract
-- is the slim common set, and code that needs more reads the underlying table.

CREATE VIEW swap_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id, pool_address, signature,
    trade_direction, amount_a, amount_b,
    reserve_a_after, reserve_b_after, timestamp
FROM meteora_damm_v2_swap_events;

CREATE VIEW liquidity_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id, pool_address, signature,
    liquidity_event_kind, amount_a, amount_b,
    reserve_a_after, reserve_b_after, position, owner, timestamp
FROM meteora_damm_v2_liquidity_events;

CREATE VIEW claim_position_fee_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id,
    pool_address,
    signature,
    position,
    owner,
    fee_a_claimed,
    fee_b_claimed,
    timestamp
FROM meteora_damm_v2_claim_position_fee_events;

CREATE VIEW claim_reward_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id,
    pool_address,
    signature,
    position,
    owner,
    mint_reward,
    reward_index,
    total_reward,
    timestamp
FROM meteora_damm_v2_claim_reward_events;

-- One row per protocol-fee claim (028). No reader yet — an indexed trace.
CREATE VIEW claim_protocol_fee_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id,
    pool_address,
    signature,
    token_a_amount,
    token_b_amount,
    timestamp
FROM meteora_damm_v2_claim_protocol_fee_events;

-- One row per reward slot opened (029).
CREATE VIEW initialize_reward_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id,
    pool_address,
    signature,
    reward_mint,
    funder,
    creator,
    reward_index,
    reward_duration,
    timestamp
FROM meteora_damm_v2_initialize_reward_events;

-- One row per farm funding (030). The rate columns stay Q64.64 here too — the
-- VIEW does not rescale.
CREATE VIEW fund_reward_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id,
    pool_address,
    signature,
    funder,
    mint_reward,
    reward_index,
    amount,
    transfer_fee_excluded_amount_in,
    reward_duration_end,
    pre_reward_rate,
    post_reward_rate,
    timestamp
FROM meteora_damm_v2_fund_reward_events;

-- One row per ineligible-reward withdrawal (031).
CREATE VIEW withdraw_ineligible_reward_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id,
    pool_address,
    signature,
    reward_mint,
    amount,
    timestamp
FROM meteora_damm_v2_withdraw_ineligible_reward_events;

-- One row per re-pacing (032).
CREATE VIEW update_reward_duration_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id,
    pool_address,
    signature,
    reward_index,
    old_reward_duration,
    new_reward_duration,
    timestamp
FROM meteora_damm_v2_update_reward_duration_events;

-- One row per funder hand-over (033).
CREATE VIEW update_reward_funder_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id,
    pool_address,
    signature,
    reward_index,
    old_funder,
    new_funder,
    timestamp
FROM meteora_damm_v2_update_reward_funder_events;

-- One row per dead-liquidity reward withdrawal (034). Deliberately NOT unioned
-- with withdraw_ineligible_reward_events: the two describe different facts and
-- a merged VIEW would erase that.
CREATE VIEW withdraw_dead_liquidity_reward_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id,
    pool_address,
    signature,
    reward_mint,
    amount,
    timestamp
FROM meteora_damm_v2_withdraw_dead_liquidity_reward_events;

-- One row per split (035). Slim on purpose: the identity of the transfer
-- (who → who, which positions, how much liquidity moved) is the cross-protocol
-- concept. The post-split position states and the requested numerators stay
-- protocol-specific — read the table for those.
CREATE VIEW split_position_events AS
SELECT
    'meteora_damm_v2'::TEXT AS protocol,
    id,
    pool_address,
    signature,
    first_owner,
    second_owner,
    first_position,
    second_position,
    split_permanent_locked_liquidity,
    split_unlocked_liquidity,
    split_vested_liquidity,
    timestamp
FROM meteora_damm_v2_split_position_events;

-- ⚠️ The only place this file is not a pure squash: `swap_events` and
-- `liquidity_events` are granted here, and the 42 migrations it replaces ended
-- without that grant.
--
-- Migration 001 gave all four ring-1 VIEWs to yog_api; migration 014 dropped
-- and recreated those two — to take the denormalised mint columns out — and did
-- not restate it. The ten added later (028-035) all carry theirs, so these two
-- were the only ones of the twelve whose SELECT was written nowhere.
--
-- Nothing broke, and that is the problem. `setup_roles.sql` grants SELECT on
-- future tables and views to yog_api by default, so the API could always read
-- both; and an ACL does not keep the origin of a right — Postgres records
-- "yog_api has SELECT", not where it came from. Neither `pg_dump` nor a schema
-- diff can see the loss, which is why it went unnoticed for two months: a
-- privilege that worked by default rather than by decision, the exact shape the
-- role model exists to prevent.
--
-- `tests/privileges.rs` found it during the squash. Its `sqlx::test` databases
-- have no default privileges — the migrations run as the connecting user, not
-- as yog_migrate — so its matrix sees only what a migration actually emitted.
--
-- The opposite fix was available and rejected: drop the ten explicit grants and
-- let the defaults carry everything. Explicit grants are the only thing that
-- matrix can assert, and the only thing that keeps the intent reviewable next
-- to the object it applies to.
GRANT SELECT ON swap_events, liquidity_events,
                claim_position_fee_events, claim_reward_events,
                claim_protocol_fee_events,
                initialize_reward_events, fund_reward_events,
                withdraw_ineligible_reward_events,
                update_reward_duration_events, update_reward_funder_events,
                withdraw_dead_liquidity_reward_events,
                split_position_events
            TO yog_api;


-- ============================================================================
-- §15 — analytical VIEWs: the USD valuations, in versioned SQL
--     [019, 020, 021 (+041), 023, 024, 025]
-- ============================================================================
-- These exist to keep large analytical SQL OUT of Rust string literals and in
-- versioned SQL — and to DRY it: each of them was, before, copy-pasted across
-- two or more read paths. The Rust queries collapse to a slim SELECT that the
-- sqlx macro still verifies against the view's columns.
--
-- None is parameterized (a VIEW cannot take args): each values every pool /
-- every bucket, and callers filter with `WHERE pool_address = … AND bucket > …`.
-- Postgres pushes those predicates down into the underlying tables and caggs,
-- so a single-pool read only touches that pool's rows. A plain VIEW gives no
-- performance gain of its own — it is inlined, same plan. It is chosen for
-- readability; the perf tool is the materialization of §13.
--
-- All are definer's-rights views owned by yog_migrate: a reading role needs
-- only SELECT on the view, not on the underlying caggs / token_prices.


-- ── meteora_damm_v2_pool_hourly_activity (019) ──────────────────────────────
-- The per-(pool, hour) USD valuation of the four hourly aggregates. Before it,
-- the as-of-bucket trade-time valuation (the LATERAL token_prices +
-- POWER(10, decimals) joins) was copy-pasted in BOTH
-- `pool_analytics.batch_compute` (24h roll-up) and `pool_analytics.history`
-- (per-bucket series).
--
-- A pool whose mints are not resolved yet drops out (INNER JOIN on
-- token_metadata) → no row. Reward claims are valued by their own reward mint
-- and summed across mints per bucket.
CREATE VIEW meteora_damm_v2_pool_hourly_activity AS
WITH pool_tokens AS (
    SELECT p.pool_address, p.token_a_mint, p.token_b_mint,
           tma.decimals AS dec_a, tmb.decimals AS dec_b
    FROM pools p
    JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
    JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
),
swap_v AS (
    SELECT h.pool_address, h.bucket,
        (COALESCE(h.volume_in_a, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
      + (COALESCE(h.volume_in_b, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd AS volume_usd,
        (COALESCE(h.fee_in_a, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
      + (COALESCE(h.fee_in_b, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd AS fees_usd,
        (COALESCE(h.protocol_fee_in_a, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
      + (COALESCE(h.protocol_fee_in_b, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd AS protocol_fees_usd,
        h.swap_count
    FROM meteora_damm_v2_swap_events_hourly h
    JOIN pool_tokens pt ON pt.pool_address = h.pool_address
    LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_a_mint::TEXT AND fetched_at <= h.bucket ORDER BY fetched_at DESC LIMIT 1) pa ON true
    LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_b_mint::TEXT AND fetched_at <= h.bucket ORDER BY fetched_at DESC LIMIT 1) pb ON true
),
liq_v AS (
    SELECT h.pool_address, h.bucket,
        (COALESCE(h.amount_a_added, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
      + (COALESCE(h.amount_b_added, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd AS liquidity_added_usd,
        (COALESCE(h.amount_a_removed, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
      + (COALESCE(h.amount_b_removed, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd AS liquidity_removed_usd
    FROM meteora_damm_v2_liquidity_events_hourly h
    JOIN pool_tokens pt ON pt.pool_address = h.pool_address
    LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_a_mint::TEXT AND fetched_at <= h.bucket ORDER BY fetched_at DESC LIMIT 1) pa ON true
    LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_b_mint::TEXT AND fetched_at <= h.bucket ORDER BY fetched_at DESC LIMIT 1) pb ON true
),
pos_fee_v AS (
    SELECT h.pool_address, h.bucket,
        (COALESCE(h.fee_a_claimed, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
      + (COALESCE(h.fee_b_claimed, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd AS fees_claimed_usd
    FROM meteora_damm_v2_claim_position_fee_events_hourly h
    JOIN pool_tokens pt ON pt.pool_address = h.pool_address
    LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_a_mint::TEXT AND fetched_at <= h.bucket ORDER BY fetched_at DESC LIMIT 1) pa ON true
    LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_b_mint::TEXT AND fetched_at <= h.bucket ORDER BY fetched_at DESC LIMIT 1) pb ON true
),
reward_v AS (
    SELECT h.pool_address, h.bucket,
        SUM((COALESCE(h.total_reward, 0)::NUMERIC / POWER(10::NUMERIC, tmr.decimals)) * pr.price_usd) AS rewards_claimed_usd
    FROM meteora_damm_v2_claim_reward_events_hourly h
    JOIN token_metadata tmr ON tmr.mint = h.mint_reward::TEXT
    LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = h.mint_reward::TEXT AND fetched_at <= h.bucket ORDER BY fetched_at DESC LIMIT 1) pr ON true
    GROUP BY h.pool_address, h.bucket
),
buckets AS (
    SELECT pool_address, bucket FROM swap_v
    UNION SELECT pool_address, bucket FROM liq_v
    UNION SELECT pool_address, bucket FROM pos_fee_v
    UNION SELECT pool_address, bucket FROM reward_v
)
SELECT
    b.pool_address,
    b.bucket,
    s.volume_usd,
    s.fees_usd,
    s.protocol_fees_usd,
    s.swap_count,
    l.liquidity_added_usd,
    l.liquidity_removed_usd,
    pf.fees_claimed_usd,
    rw.rewards_claimed_usd
FROM buckets b
LEFT JOIN swap_v s    ON s.pool_address = b.pool_address AND s.bucket = b.bucket
LEFT JOIN liq_v l     ON l.pool_address = b.pool_address AND l.bucket = b.bucket
LEFT JOIN pos_fee_v pf ON pf.pool_address = b.pool_address AND pf.bucket = b.bucket
LEFT JOIN reward_v rw  ON rw.pool_address = b.pool_address AND rw.bucket = b.bucket;

GRANT SELECT ON meteora_damm_v2_pool_hourly_activity TO yog_api;


-- ── pool_current_tvl (020) ──────────────────────────────────────────────────
-- Per-pool CURRENT TVL in USD. DRYs the `reserve × most-recent-price` math that
-- was copy-pasted in `pool_analytics.batch_compute` and `global_analytics`.
--
-- NOT protocol-prefixed: it reads only protocol-neutral tables, unlike the
-- activity view above which reads the Meteora aggregates.
--
-- Current price, NOT as-of a bucket — this is a live snapshot, not history.
-- `tvl_usd` is NULL when either token has no known price (the arithmetic
-- propagates NULL), which is exactly what the partial-coverage callers expect:
-- the priced-pool count is `COUNT(*) FILTER (WHERE tvl_usd IS NOT NULL)`.
CREATE VIEW pool_current_tvl AS
SELECT
    pcs.pool_address,
    (
        (pcs.reserve_a::NUMERIC / POWER(10::NUMERIC, tma.decimals)) * tpa.price_usd
      + (pcs.reserve_b::NUMERIC / POWER(10::NUMERIC, tmb.decimals)) * tpb.price_usd
    ) AS tvl_usd
FROM pool_current_state pcs
JOIN pools p ON p.pool_address = pcs.pool_address
JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = p.token_a_mint::TEXT
    ORDER BY fetched_at DESC LIMIT 1
) tpa ON true
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = p.token_b_mint::TEXT
    ORDER BY fetched_at DESC LIMIT 1
) tpb ON true;

GRANT SELECT ON pool_current_tvl TO yog_api;
-- The TVL-drain detector joins the flow with the pool's current TVL (025).
GRANT SELECT ON pool_current_tvl TO yog_signals;


-- ── meteora_damm_v2_liquidity_events_valued (021, extended by 041) ──────────
-- A per-event trade-time USD value on each liquidity event, so the pool-detail
-- liquidity table can show a "Value (USD)" column. The paginated read has TWO
-- cursor paths — forward and backward — so an inline join would be duplicated.
--
-- The joins are LEFT (unlike the views above): the event row must ALWAYS appear
-- in the table — only its `value_usd` is optional. It is NULL when either token
-- has no known price as-of the event, or the pool's mints / decimals are not
-- resolved yet — "factual or absent, never fake"; the frontend renders "—".
--
-- This is the only VIEW the domain rebuilds into a typed event, so its
-- `TryFrom<Row>` needs the position-in-chain triple. Those three columns sit at
-- the END of the list: `CREATE OR REPLACE VIEW` can only extend, never reorder,
-- and the `query_as!` calls reading it map by position (041).
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
    ) AS value_usd,
    le.slot,
    le.event_index,
    le.transaction_index
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


-- ── meteora_damm_v2_pool_hourly_flow (023) ──────────────────────────────────
-- Directional per-(pool, hour) USD swap volume, feeding the Signal Engine's
-- flow-imbalance detector. Same valuation as the activity view's `swap_v` CTE,
-- but the two trade directions are kept SEPARATE — 019 sums them into a single
-- `volume_usd`, and a flow imbalance needs each side on its own.
--
-- A separate, single-purpose view rather than more columns on 019: 019's
-- contract is the four-aggregate activity roll-up; this stays focused.
CREATE VIEW meteora_damm_v2_pool_hourly_flow AS
WITH pool_tokens AS (
    SELECT p.pool_address, p.token_a_mint, p.token_b_mint,
           tma.decimals AS dec_a, tmb.decimals AS dec_b
    FROM pools p
    JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
    JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
)
SELECT
    h.pool_address,
    h.bucket,
    (COALESCE(h.volume_in_a, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
        AS volume_a_to_b_usd,
    (COALESCE(h.volume_in_b, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd
        AS volume_b_to_a_usd
FROM meteora_damm_v2_swap_events_hourly h
JOIN pool_tokens pt ON pt.pool_address = h.pool_address
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = pt.token_a_mint::TEXT AND fetched_at <= h.bucket
    ORDER BY fetched_at DESC LIMIT 1
) pa ON true
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = pt.token_b_mint::TEXT AND fetched_at <= h.bucket
    ORDER BY fetched_at DESC LIMIT 1
) pb ON true;

GRANT SELECT ON meteora_damm_v2_pool_hourly_flow TO yog_signals;


-- ── pool_price_snapshot (024) ───────────────────────────────────────────────
-- Per-pool CURRENT price inputs from both sources, feeding the Signal Engine's
-- price-oracle-deviation detector: the on-chain side (the pool's
-- `last_sqrt_price`, decoded to a spot price in Rust — the Q64.64
-- interpretation is protocol-specific, so SQL only carries the raw value and
-- the `protocol` discriminator) and the oracle side (each token's most recent
-- `token_prices` row, with its `fetched_at` so the reader can gate on price
-- staleness). `last_swap_at` rides along for the symmetric gate: a pool that
-- has not traded recently has an equally stale spot price.
--
-- INNER joins drop pools that cannot be compared: unresolved mints/decimals, no
-- price observation for either token, or no swap observed yet.
CREATE VIEW pool_price_snapshot AS
SELECT
    pcs.pool_address,
    pcs.protocol,
    pcs.last_sqrt_price,
    pcs.last_swap_at,
    tma.decimals   AS decimals_a,
    tmb.decimals   AS decimals_b,
    tpa.price_usd  AS price_a_usd,
    tpa.fetched_at AS price_a_fetched_at,
    tpb.price_usd  AS price_b_usd,
    tpb.fetched_at AS price_b_fetched_at
FROM pool_current_state pcs
JOIN pools p ON p.pool_address = pcs.pool_address
JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
JOIN LATERAL (
    SELECT price_usd, fetched_at FROM token_prices
    WHERE mint = p.token_a_mint::TEXT
    ORDER BY fetched_at DESC LIMIT 1
) tpa ON true
JOIN LATERAL (
    SELECT price_usd, fetched_at FROM token_prices
    WHERE mint = p.token_b_mint::TEXT
    ORDER BY fetched_at DESC LIMIT 1
) tpb ON true
WHERE pcs.last_sqrt_price IS NOT NULL
  AND pcs.last_swap_at IS NOT NULL;

GRANT SELECT ON pool_price_snapshot TO yog_signals;


-- ── meteora_damm_v2_pool_hourly_liquidity_flow (025) ────────────────────────
-- Per-(pool, hour) USD liquidity flow split by direction, feeding the Signal
-- Engine's TVL-drain detector.
--
-- Each direction sums BOTH token legs (an add/remove touches both sides
-- together). A missing price as-of the bucket propagates NULL through the whole
-- expression — deliberate: a partially-priced flow would silently undervalue
-- the drain ratio, and the detector's TVL guard skips unpriced pools anyway
-- (pool_current_tvl is NULL for them too), so both sides of the comparison go
-- absent together rather than half-fake.
CREATE VIEW meteora_damm_v2_pool_hourly_liquidity_flow AS
WITH pool_tokens AS (
    SELECT p.pool_address, p.token_a_mint, p.token_b_mint,
           tma.decimals AS dec_a, tmb.decimals AS dec_b
    FROM pools p
    JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
    JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
)
SELECT
    h.pool_address,
    h.bucket,
    (COALESCE(h.amount_a_added, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
  + (COALESCE(h.amount_b_added, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd
        AS added_usd,
    (COALESCE(h.amount_a_removed, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
  + (COALESCE(h.amount_b_removed, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd
        AS removed_usd
FROM meteora_damm_v2_liquidity_events_hourly h
JOIN pool_tokens pt ON pt.pool_address = h.pool_address
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = pt.token_a_mint::TEXT AND fetched_at <= h.bucket
    ORDER BY fetched_at DESC LIMIT 1
) pa ON true
LEFT JOIN LATERAL (
    SELECT price_usd FROM token_prices
    WHERE mint = pt.token_b_mint::TEXT AND fetched_at <= h.bucket
    ORDER BY fetched_at DESC LIMIT 1
) pb ON true;

GRANT SELECT ON meteora_damm_v2_pool_hourly_liquidity_flow TO yog_signals;
