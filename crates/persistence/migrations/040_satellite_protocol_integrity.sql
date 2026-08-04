-- ============================================================================
-- 040 — the pool↔protocol agreement, written into the schema
-- ============================================================================
-- Three tables hang off `pools` and carry, or imply, a protocol of their own:
-- the two pool-properties satellites (`meteora_damm_v2_pool_properties` 036,
-- `meteora_dlmm_pool_properties` 039) and the state projection
-- `pool_current_state` (001). All three reference `pools (pool_address)`
-- **alone**, so nothing in the schema says their protocol must be the one the
-- registry records.
--
-- This migration says it: a composite key on `(pool_address, protocol)`.
--
-- ## This is defence in depth, not a hole being plugged
--
-- The live write paths already agree with the registry, and this migration does
-- not fix a bug observed in production:
--
--   * **Satellites.** `PoolAccountResolver::set_pool_account` is the single
--     write path, behind a queue already scoped by protocol
--     (`list_unresolved … WHERE p.protocol = $2`). Each resolver rejects a
--     payload of another protocol. And upstream of both,
--     `context/src/workers/pool_account.rs` skips any pool whose *decoded
--     account* disagrees with the queue's protocol — which is precisely the
--     "right payload, wrong pool" case, caught with a warn before the resolver
--     is ever called.
--   * **`pool_current_state`.** `discover_pool` and
--     `update_pool_current_state_from_*` are handed the same `Self::PROTOCOL`
--     constant from the same sub-persistor call site, so the two labels are
--     written from one value and cannot drift.
--
-- What the constraint buys is what those guards cannot: they live in loops and
-- call sequences no compiler protects, and they hold only for *today's* callers.
-- A refactor of the worker, a second writer, a repair script run by hand in
-- psql, a future protocol whose author copies the repository but not the guard —
-- none of those are hypothetical in a codebase that adds a protocol per
-- quarter. The schema outlives the call graph.
--
-- The one guard that *was* compiler-enforced stopped being so: cp-amm's
-- irrefutable `let` broke when DLMM added a second variant, but from two
-- variants on `let … else { Err }` accepts a third without complaint. That is
-- the shape of the whole argument — the discipline is real, it is simply not
-- structural, and it is recopied by hand for each new protocol.
--
-- ## Why a generated column rather than a CHECK
--
-- A `CHECK (protocol = 'meteora_damm_v2')` constrains the satellite row against
-- itself. It never looks at `pools.protocol`, so on its own it enforces nothing
-- across the two tables. The linkage has to be a foreign key on
-- `(pool_address, protocol)` — and the `CHECK` would only be the thing pinning
-- the satellite's own column to a constant.
--
-- `GENERATED ALWAYS AS ('…') STORED` does that pinning better:
--
--   * the constant *is* the check — no second constraint to keep in sync;
--   * no `UPDATE` back-fill to write (see the caveat below);
--   * no INSERT anywhere has to supply it, so **not one line of Rust changes**;
--   * Postgres refuses `INSERT … (pool_address, protocol, …)` outright, so a
--     writer cannot lie about the protocol even deliberately.
--
-- This applies to the two satellites, whose protocol is a constant of the table.
-- `pool_current_state` is different: its `protocol` column already exists and
-- holds real per-row data written by the indexer, so there the composite key is
-- a bare FK swap — no column to add, nothing to rewrite.
--
-- ### Caveat: "no back-fill" is not "no rewrite"
--
-- `ADD COLUMN … GENERATED ALWAYS … STORED` rewrites the whole table under
-- `ACCESS EXCLUSIVE`. The difference with an `UPDATE` back-fill is that nobody
-- has to write it, not that it does not happen. On the few hundred rows these
-- tables hold it is invisible; anyone applying this pattern to a large table
-- should size the lock first. The recipe for the third satellite below avoids it
-- entirely by putting the column in the `CREATE TABLE`.
--
-- ## The redundant UNIQUE on `pools` is the price of the composite key
--
-- `pool_address` is already the primary key, so `UNIQUE (pool_address, protocol)`
-- adds no new guarantee about `pools` — it exists because a foreign key needs a
-- unique constraint covering exactly its referenced columns. One extra index on
-- a table of a few hundred rows, and `pools` is not a hypertable, so there is no
-- partitioning-column constraint to satisfy.
--
-- It also buys something real in the other direction: `pools.protocol` can no
-- longer change under a dependent row. Today nothing updates that column
-- (`PgPoolRepository::upsert` writes it on INSERT and its ON CONFLICT touches
-- only `last_seen_at`), so this blocks a bug rather than a workflow.
--
-- ## For the third satellite
--
-- Orca, Raydium, whatever comes next: two lines in its own `CREATE TABLE` — the
-- generated column, and the composite FK instead of the single-column one. It
-- replaces "remember to copy the Rust guard" with something the schema enforces
-- whether or not anyone remembered.
--
-- ## If this migration fails
--
-- `ADD CONSTRAINT … FOREIGN KEY` validates existing rows. A failure here means
-- the database already holds a dependent row whose protocol disagrees with the
-- registry — real corruption, exactly what the constraint exists to surface.
-- Migrations are forward-only: fix the data, then re-run. Do not weaken the
-- constraint.

-- ── The referenced key ──────────────────────────────────────────────────────
ALTER TABLE pools
    ADD CONSTRAINT pools_pool_address_protocol_key UNIQUE (pool_address, protocol);


-- ── cp-amm satellite (migration 036) ────────────────────────────────────────
ALTER TABLE meteora_damm_v2_pool_properties
    ADD COLUMN protocol TEXT NOT NULL
        GENERATED ALWAYS AS ('meteora_damm_v2') STORED;

-- The single-column FK is subsumed by the composite one — same parent row, same
-- cascade — so keeping both would only mean two referential checks per write.
ALTER TABLE meteora_damm_v2_pool_properties
    DROP CONSTRAINT meteora_damm_v2_pool_properties_pool_address_fkey,
    ADD  CONSTRAINT meteora_damm_v2_pool_properties_pool_fkey
        FOREIGN KEY (pool_address, protocol)
        REFERENCES pools (pool_address, protocol) ON DELETE CASCADE;


-- ── DLMM satellite (migration 039) ──────────────────────────────────────────
ALTER TABLE meteora_dlmm_pool_properties
    ADD COLUMN protocol TEXT NOT NULL
        GENERATED ALWAYS AS ('meteora_dlmm') STORED;

ALTER TABLE meteora_dlmm_pool_properties
    DROP CONSTRAINT meteora_dlmm_pool_properties_pool_address_fkey,
    ADD  CONSTRAINT meteora_dlmm_pool_properties_pool_fkey
        FOREIGN KEY (pool_address, protocol)
        REFERENCES pools (pool_address, protocol) ON DELETE CASCADE;


-- ── pool_current_state (migration 001) ──────────────────────────────────────
-- The cheapest of the three, and the one whose redundancy was already there:
-- `pool_current_state.protocol` has duplicated `pools.protocol` since 001, is
-- NOT NULL, is indexed, and nothing tied the two together. No column to add, no
-- rewrite — the FK swap alone.
--
-- The projection is cross-protocol by design (one row per pool, whatever the
-- protocol), so there is no invariant here about *which* pools may have a row.
-- The invariant is that the two labels agree, and it is exactly the one above.
--
-- Safe for the ingestion hot path: the upsert's `ON CONFLICT … SET protocol =
-- EXCLUDED.protocol` re-states the same value, and Postgres skips the
-- referential check when the key is unchanged.
ALTER TABLE pool_current_state
    DROP CONSTRAINT pool_current_state_pool_address_fkey,
    ADD  CONSTRAINT pool_current_state_pool_fkey
        FOREIGN KEY (pool_address, protocol)
        REFERENCES pools (pool_address, protocol) ON DELETE CASCADE;

-- No new GRANT. Privileges here are table-level (001, 036, 039) and already
-- cover every column; a generated column is writable by no role in any case.
