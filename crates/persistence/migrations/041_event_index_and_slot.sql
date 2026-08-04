-- ============================================================================
-- 041 — where an event sits in the chain: slot, event_index,
--       transaction_index, and the unique key that rests on them
-- ============================================================================
-- A transaction routed across several pools emits one event per hop. The
-- unique key of the 19 event tables was `(signature, timestamp)` — no
-- intra-transaction discriminant — and the INSERT is `ON CONFLICT DO NOTHING`:
-- everything but the first to land was dropped, with no error, no log, no
-- metric.
--
-- Measured on 4 August 2026 across three mainnet pools and 482 emissions: 29
-- losses attributable to the key, 0 to transport. The rate rises with how
-- central the pool is to routing — 7,96 % on SOL-USDC, 5,81 % on NEST-SOL,
-- 3,39 % on AVICI-USDC. The defect therefore hits hardest where the data
-- matters most, and is quietest on tail pools.
--
-- ## What the fix buys, and what it does not
--
-- **Completeness**, not volume. The lost legs weigh 0,04× the average (1 $
-- against 32 $): they are ~8 % of the events for ~0,4 % of the volume. Do not
-- read those percentages as a revenue shortfall.
--
-- ## `event_index` indexes raw payloads, not recognised events
--
-- It numbers the transaction's Anchor self-CPIs as `extract_anchor_event_cpis`
-- returns them, including those whose discriminator is not (yet) implemented.
-- Numbering only the recognised events would shift every index already stored
-- the day one more discriminator is decoded, and a replay would insert
-- duplicates instead of being idempotent. Corollary: the filter in
-- `try_extract_self_cpi_data` is frozen by contract.
--
-- ## Existing rows: `0`, then a rank where `0` is not enough
--
-- `slot` and `event_index` are `NOT NULL DEFAULT 0` for the duration of the
-- backfill, then the DEFAULT is dropped. `0` is honest on the fourteen tables
-- whose key was `(signature, timestamp)`: their old unique index already
-- forbade two rows per transaction, so the survivor *was* alone.
--
-- The other five — those with `reward_index` or `second_position` in their key
-- — legitimately hold several rows per `(signature, timestamp)`. A uniform `0`
-- would make them identical and creating the unique index would **fail**. They
-- are therefore renumbered with `row_number()` first. That rank is not the real
-- on-chain index (it is not recoverable without a replay): it preserves the
-- distinction, nothing more.
--
-- Consequence worth knowing: replaying a transaction predating this migration
-- would recompute the real index and so would NOT conflict with the
-- retro-numbered row — it would insert a duplicate. No replay path exists today
-- (ingestion is subscription-based, the StreamPoller is dormant); the day one is
-- wired, it must start after 041.
--
-- **`slot` stays at `0` everywhere**, and that is safer than it looks: `0` is
-- Solana's genesis slot (June 2020) while cp-amm was deployed in 2025 and real
-- slots sit around 300 million. So it is not one plausible integer among others
-- but an **impossible** value — a true sentinel. The ordering guard of ticket 04,
-- which compares `(slot, event_index)` tuples, inherits a guarantee rather than
-- a premise: every row predating this migration sorts before everything that
-- follows — which is correct, they *are* the oldest — and none can be mistaken
-- for a recent one.
--
-- NULL was not an option: it would break the tuple comparison of the ordering
-- guard (next migration, ticket 04).
--
-- Dropping the DEFAULT afterwards is deliberate: an insertion path that forgot
-- the column must fail loudly rather than inherit a silent `0`. That is the
-- lesson of the `DO NOTHING` this migration corrects.
--
-- ## Widths follow the schema's convention, not the size of the values
--
-- `event_index` holds a `u16` in the domain, so **INTEGER** and not SMALLINT: a
-- `u16` does not fit in a SMALLINT (32 767 < 65 535), and the house keeps those
-- columns as INTEGER so the write conversion is total (`i32::from`) rather than
-- fallible — the rule `convert_i32_to_u16` documents, and `bin_step` already
-- follows. Same reasoning one notch up for `transaction_index`, a `u32`, as
-- **BIGINT**. The real values would fit in two bytes; that is not the criterion.
--
-- `transaction_index` is created **nullable and empty**. `getTransaction`
-- (Helius) does not return the field — verified live and against the 6 fixtures
-- in the repository. The key reachable today is therefore `(slot, event_index)`,
-- which orders within a transaction and between slots, but not between two
-- transactions of one slot. The column exists so that the gRPC/Geyser migration
-- — where the transaction update carries its `index` natively — gives the field
-- its meaning with no second migration and no code change.
--
-- ## Index names are not hard-coded, and that is necessary
--
-- Postgres truncates auto-generated names at 63 characters, and two of ours
-- already collided:
-- `meteora_damm_v2_update_reward_signature_reward_index_timest_idx` belongs to
-- `update_reward_duration_events`, and its sibling `…_times_idx1` to
-- `update_reward_funder_events`. The `1` suffix depends on creation order, hence
-- on migration order: a literal name would be right here and wrong on a fresh
-- database. The `DO` block finds the unique index carrying `signature` on each
-- table instead.
--
-- ## The five existing discriminants leave the key, not the table
--
-- `fund_reward`, `initialize_reward`, `update_reward_duration` and
-- `update_reward_funder` were on `(signature, reward_index, timestamp)`,
-- `split_position` on `(signature, second_position, timestamp)`. `event_index`
-- is strictly more general: those columns remain as **data** (they carry
-- business meaning) but leave the key, which brings 19 tables down to a single
-- rule instead of maintaining three.
--
-- `claim_reward_events` carried `reward_index` **without** having it in its key:
-- a latent instance of the same bug, in a table that can legitimately repeat
-- within one transaction. The uniform fix covers it.
--
-- ## No GRANT here
--
-- The 19 tables already hold a `GRANT SELECT, INSERT, UPDATE … TO yog_indexer`
-- at **table** level, which covers columns added later. The caveat in
-- `migrations/README.md` about per-column grants concerns only `yog_context` on
-- `pools`, outside this migration.

DO $$
DECLARE
    -- The 19 DAMM v2 event tables. A new event table created after this
    -- migration is born with the three columns and the right key (see the
    -- template in `migrations/README.md`), so it has no business here.
    tables CONSTANT TEXT[] := ARRAY[
        'meteora_damm_v2_claim_position_fee_events',
        'meteora_damm_v2_claim_protocol_fee_events',
        'meteora_damm_v2_claim_reward_events',
        'meteora_damm_v2_close_position_events',
        'meteora_damm_v2_create_position_events',
        'meteora_damm_v2_fund_reward_events',
        'meteora_damm_v2_initialize_pool_events',
        'meteora_damm_v2_initialize_reward_events',
        'meteora_damm_v2_liquidity_events',
        'meteora_damm_v2_lock_position_events',
        'meteora_damm_v2_permanent_lock_position_events',
        'meteora_damm_v2_set_pool_status_events',
        'meteora_damm_v2_split_position_events',
        'meteora_damm_v2_swap_events',
        'meteora_damm_v2_update_pool_fees_events',
        'meteora_damm_v2_update_reward_duration_events',
        'meteora_damm_v2_update_reward_funder_events',
        'meteora_damm_v2_withdraw_dead_liquidity_reward_events',
        'meteora_damm_v2_withdraw_ineligible_reward_events'
    ];
    tbl       TEXT;
    old_index TEXT;
BEGIN
    FOREACH tbl IN ARRAY tables LOOP
        EXECUTE format(
            'ALTER TABLE %I
                 ADD COLUMN slot              BIGINT  NOT NULL DEFAULT 0,
                 ADD COLUMN event_index       INTEGER NOT NULL DEFAULT 0,
                 ADD COLUMN transaction_index BIGINT  NULL',
            tbl);

        -- The old idempotency guard, whatever shape it had:
        -- (signature, timestamp), (signature, reward_index, timestamp) or
        -- (signature, second_position, timestamp). Found by its columns, never
        -- by its name (see the header).
        SELECT i.relname INTO old_index
        FROM pg_index x
        JOIN pg_class i  ON i.oid = x.indexrelid
        JOIN pg_class c  ON c.oid = x.indrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = 'public'
          AND c.relname = tbl
          AND x.indisunique
          AND EXISTS (
              SELECT 1
              FROM pg_attribute a
              WHERE a.attrelid = x.indrelid
                AND a.attnum   = ANY (x.indkey)
                AND a.attname  = 'signature'
          );

        IF old_index IS NULL THEN
            RAISE EXCEPTION
                'migration 041: no unique index carrying signature on %', tbl;
        END IF;

        EXECUTE format('DROP INDEX %I', old_index);

        -- Renumbering the rows already stored, without which the unique index
        -- below cannot be created on the five tables that had their own
        -- discriminant: two reward slots funded by one transaction legitimately
        -- coexist there under `(signature, timestamp)`, and `DEFAULT 0` would
        -- make them identical.
        --
        -- The rank is NOT the event's real on-chain index — that one is not
        -- recoverable without a replay. It preserves exactly what the old key
        -- guaranteed: the distinction. On the other fourteen tables the old
        -- unique index already forbade any duplicate, so each group holds one
        -- row and the UPDATE touches nothing.
        EXECUTE format(
            'UPDATE %I e
                SET event_index = r.ordinal
               FROM (
                   SELECT id,
                          timestamp AS ts,
                          row_number() OVER (
                              PARTITION BY signature, timestamp ORDER BY id
                          ) - 1 AS ordinal
                     FROM %I
               ) r
              WHERE e.id = r.id AND e.timestamp = r.ts AND r.ordinal > 0',
            tbl, tbl);

        EXECUTE format(
            'CREATE UNIQUE INDEX ON %I (signature, event_index, timestamp)',
            tbl);

        -- The DEFAULT only ever existed to fill the rows already stored.
        EXECUTE format(
            'ALTER TABLE %I
                 ALTER COLUMN slot        DROP DEFAULT,
                 ALTER COLUMN event_index DROP DEFAULT',
            tbl);
    END LOOP;
END $$;

-- ============================================================================
-- The valued liquidity-events VIEW exposes the three columns
-- ============================================================================
-- `meteora_damm_v2_liquidity_events_valued` (migration 021) is the only VIEW
-- the domain rebuilds into an event type: its `TryFrom<Row>` must therefore be
-- able to fill the three fields. The columns are **appended at the end of the
-- list** because `CREATE OR REPLACE VIEW` can only extend, never reorder — and
-- the `query_as!` calls reading it map by position.
--
-- The cross-protocol VIEWs (`swap_events`, `liquidity_events`, …) are
-- unchanged: their contract is the slim common column set, and none of their
-- readers rebuilds a typed event.
CREATE OR REPLACE VIEW meteora_damm_v2_liquidity_events_valued AS
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

-- ============================================================================
-- Verification — the migration observes its own result
-- ============================================================================
-- The fix spans 19 tables through a loop: a table missing from the list, or a
-- leftover index, would show up in production as one more silent loss. Better
-- to fail here.
DO $$
DECLARE
    n_new INT;
    n_old INT;
BEGIN
    SELECT
        count(*) FILTER (WHERE cols = ARRAY['signature', 'event_index', 'timestamp']),
        count(*) FILTER (WHERE cols <> ARRAY['signature', 'event_index', 'timestamp'])
    INTO n_new, n_old
    FROM (
        SELECT (
            SELECT array_agg(a.attname::TEXT ORDER BY k.ord)
            FROM unnest(x.indkey::SMALLINT[]) WITH ORDINALITY AS k(attnum, ord)
            JOIN pg_attribute a
              ON a.attrelid = x.indrelid AND a.attnum = k.attnum
        ) AS cols
        FROM pg_index x
        JOIN pg_class c     ON c.oid = x.indrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = 'public'
          AND c.relname LIKE 'meteora\_damm\_v2\_%\_events'
          AND x.indisunique
          AND NOT x.indisprimary
    ) AS unique_indexes;

    IF n_new <> 19 OR n_old <> 0 THEN
        RAISE EXCEPTION
            'migration 041: expected 19 (signature, event_index, timestamp) indexes and 0 leftover, got % and %',
            n_new, n_old;
    END IF;
END $$;
