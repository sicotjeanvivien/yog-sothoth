-- ============================================================================
-- 040 — l'invariant satellite↔protocole, écrit dans le schéma
-- ============================================================================
-- A satellite row must not exist for a pool of another protocol. Until now that
-- rule lived nowhere: `meteora_damm_v2_pool_properties` and
-- `meteora_dlmm_pool_properties` both reference `pools (pool_address)` **alone**,
-- so a cp-amm property row for a `protocol = 'meteora_dlmm'` pool was accepted
-- without a murmur.
--
-- ## What held it, and why that stopped being enough
--
-- Three layers of application discipline: one write path
-- (`PoolAccountResolver::set_pool_account`), a worker routing by
-- `PoolAccountProperties::protocol`, and each resolver rejecting a foreign
-- payload. The third layer was compiler-enforced exactly once — cp-amm's
-- irrefutable `let` broke when DLMM added a second variant. From two variants
-- on, `let … else { Err }` accepts a third without complaint: nothing makes the
-- next author write the guard.
--
-- More to the point, that guard answers a different question. It rejects a
-- *payload* of the wrong protocol. It says nothing about a **right payload on a
-- wrong pool** — the cp-amm resolver handed a cp-amm payload for a pool the
-- registry labels DLMM writes it happily. That is the hole, and only the
-- database can close it.
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
--   * no back-fill — `ALTER TABLE … ADD COLUMN` computes it for existing rows,
--     which is what made this migration cheap on a table already carrying data;
--   * no INSERT anywhere has to supply it, so **not one line of Rust changes**;
--   * Postgres refuses `INSERT … (pool_address, protocol, …)` outright, so a
--     writer cannot lie about the protocol even deliberately.
--
-- ## The redundant UNIQUE on `pools` is the price of the composite key
--
-- `pool_address` is already the primary key, so `UNIQUE (pool_address, protocol)`
-- adds no new guarantee about `pools` — it exists because a foreign key needs a
-- unique constraint covering exactly its referenced columns. One extra index on
-- a table of a few hundred rows.
--
-- It also buys something real in the other direction: `pools.protocol` can no
-- longer change under an existing satellite row. Today nothing updates that
-- column (`PgPoolRepository::upsert` writes it on INSERT and its ON CONFLICT
-- touches only `last_seen_at`), so this blocks a bug rather than a workflow.
--
-- ## For the third satellite
--
-- Orca, Raydium, whatever comes next: the pattern is three lines of DDL in the
-- satellite's own migration — the generated column, and the composite FK instead
-- of the single-column one. It replaces "remember to copy the Rust guard" with
-- something the schema enforces whether or not anyone remembered.
--
-- ## `pool_current_state` is deliberately untouched
--
-- It also references `pools (pool_address)` alone, and correctly so: it is
-- cross-protocol by construction, one row per pool whatever the protocol. There
-- is no invariant to write there.
--
-- ## If this migration fails
--
-- `ADD CONSTRAINT … FOREIGN KEY` validates existing rows. A failure here means
-- the database already holds a satellite row for a foreign-protocol pool — real
-- corruption, exactly what the constraint exists to surface. Migrations are
-- forward-only: fix the data, then re-run. Do not weaken the constraint.

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

-- No new GRANT. Privileges here are table-level (036, 039) and already cover
-- every column; a generated column is writable by no role in any case.
