# yog-persistence

PostgreSQL adapter for yog-sothoth.

This README is the crate-level overview. It points to the technical
architecture and migration conventions kept elsewhere, and hosts what is
specific to this crate and doesn't fit anywhere else — most notably the
`watched_pools` allowlist that bounds ingestion in v0.1.

For the crate's architecture (layout, repository pattern, sqlx offline
cache, database roles), see [`crates/README.md`](../README.md#persistence-yog-persistence).
For the migration conventions (forward-only, GRANTs per migration, the
local workflow when adding a new migration), see [`migrations/README.md`](./migrations/README.md).

---

## `watched_pools` — startup allowlist

Until the indexer runs on an upgraded RPC path (Helius `transactionSubscribe`,
Helius Startup Launchpad, or an equivalent gRPC provider), ingestion is bounded
by an allowlist of pools stored in the `watched_pools` table. The
protocol-centric architecture is preserved — the allowlist is applied as a
filter inside the dispatcher's filter chain, not as a return to static
configuration. Lifting the constraint is a matter of disabling the filter.

The rationale for the constraint, and the conditions under which it is lifted,
are summarised in the [root README's *Pool observation model*](../../README.md#pool-observation-model).
The content below is the operational reference: schema, current selection,
seed script, and administration helpers.

### Schema

| Column | Type | Purpose |
|---|---|---|
| `pool_address` | `TEXT PRIMARY KEY` | Solana pubkey of the pool |
| `protocol` | `TEXT NOT NULL` | Protocol identifier (`damm_v2`, etc.) |
| `active` | `BOOLEAN NOT NULL DEFAULT TRUE` | Whether the filter accepts events for this pool |
| `added_at` | `TIMESTAMPTZ NOT NULL DEFAULT NOW()` | When the pool was added to the allowlist |
| `note` | `TEXT` | Free-form annotation (selection rationale, edge-case marker, etc.) |

A partial index on `(pool_address) WHERE active = TRUE` keeps the lookup cheap
regardless of how many deactivated rows accumulate over time.

Deactivation uses the `active` flag rather than row deletion, to preserve
history and allow reactivation without re-selection.

### Decoupling from `pools`

There is **no foreign key** from `watched_pools.pool_address` to
`pools.pool_address`. The two tables serve different purposes:

- `pools` is a **record** — what the indexer has observed in the transaction stream.
- `watched_pools` is a **configuration** — what the indexer is authorised to ingest.

A pool can legitimately appear in `watched_pools` before it appears in `pools`
(the moment between seeding the allowlist and observing the first transaction).
Forcing a FK would either reject the seed or require pre-populating `pools`
with empty rows, both worse than the current decoupling.

### Current selection

The allowlist was seeded from the 7-day activity distribution of `swap_events`
observed during a calibration window. Pools were chosen to balance
high-signal density (top of the distribution) with edge-case diversity
(lower-activity pools for testing short-lived or thin-liquidity behaviour).

| Pool address | 7d swap count | First swap (UTC) | Last swap (UTC) | Notes |
|---|---:|---|---|---|
| `AKniRboGuKBRAUWh2QvQmMxDppcn8uzDx1LAngADJoBv` | 906 | 2026-04-22 09:02 | 2026-04-22 09:53 | High activity, short burst |
| `8DW1L4yJRm2NNygASN1nFKEXwxLurkozxuYATZCT3gpb` | 818 | 2026-04-22 09:31 | 2026-04-22 09:53 | High activity, short burst |
| `9g2wf7xTBsVxoVnypCdKrUmBtH6Ms1tSzVEJQNj86eHg` | 774 | 2026-04-22 09:43 | 2026-04-22 09:53 | High activity, very short window |
| `5BohNRJgMtSv9C4PqxhvkXL1v1j7gouBoj4usNG8LGH` | 758 | 2026-04-22 09:31 | 2026-04-22 09:53 | High activity, short burst |
| `GpnMyz78yTRiS2oBMroEKEynG7LkjWZq61aaU1MD558L` | 720 | 2026-04-21 09:24 | 2026-04-21 09:59 | High activity, previous day |
| `6bkGH5bdNWym7eP2KKDDbCt5jMn9NB1dV7dN9fbb1Bz8` | 674 | 2026-04-22 09:43 | 2026-04-22 09:53 | High activity, very short window |
| `CfpwKVuB8Y41re9U5qpYmD3oYiDijTcsHe3c3fs8GsFg` | 601 | 2026-04-22 12:23 | 2026-04-22 12:23 | Extreme burst (<1 min) |
| `AMxysMpo34c3aNb5bWW28p4AkXzWJFdM5Wdrtfmy4bMx` | 237 | 2026-04-21 09:59 | 2026-04-21 09:59 | Ephemeral, edge case |
| `EV9h8xS1yF3GJ8LnkaE65hQx5ViCSSeoVaHT6JPaVyPW` | 235 | 2026-04-21 09:24 | 2026-04-21 09:33 | Ephemeral, edge case |
| `59drqEGrECHxMkHPKcr1JZggNfPxNKsrQP5MvCBEY5av` | 234 | 2026-04-21 09:41 | 2026-04-21 09:42 | Ephemeral, edge case |

> **Note on observed activity patterns** — most pools in the selection exhibit
> burst behaviour (high swap count over a short window, then quiescence). This
> is consistent with DAMM v2 being used heavily for memecoin launches.
> Longer-lived pools will be added as the dataset grows.

### Seeding the allowlist

A SQL script at the repo root populates the 10-pool selection in development
environments:

```bash
psql "postgresql://yog:yog@localhost:5433/yog_sothoth" \
    -f scripts/seed_watched_pools.sql
```

The script is idempotent — `INSERT ... ON CONFLICT (pool_address) DO NOTHING`
— so re-running it after a partial seed or against an existing database is
safe.

Run it as the admin role rather than as `yog_indexer`: the seed adjusts the
allowlist which is configuration, not runtime data, and the convention is to
keep all configuration writes under the admin role.

### Administration helpers

These are the four operations you'll run by hand to manage the allowlist
ad-hoc. They are intended for the admin role:

```sql
-- Add a pool
INSERT INTO watched_pools (pool_address, protocol, note)
VALUES ('<pubkey>', 'damm_v2', 'manual selection: high TVL');

-- Deactivate without losing history
UPDATE watched_pools
SET active = FALSE
WHERE pool_address = '<pubkey>';

-- Reactivate
UPDATE watched_pools
SET active = TRUE
WHERE pool_address = '<pubkey>';

-- List currently active
SELECT pool_address, protocol, added_at, note
FROM watched_pools
WHERE active = TRUE
ORDER BY added_at DESC;
```

The filter is loaded once at indexer startup. Modifying `watched_pools` while
the indexer is running has no effect on the running process — restart the
indexer to pick up the change. Hot reload will land in **v0.3** when
user-managed watchlists become a first-class feature.

### Removing the constraint

The allowlist is temporary. It will be lifted once one of the following is in
place:

- **Helius `transactionSubscribe` (Developer plan)** — eliminates the HTTP
  fetch entirely; transactions arrive fully parsed in the WebSocket stream.
- **Helius Startup Launchpad** — 8 months of Business tier free (LaserStream
  mainnet, 200 RPS).
- **An equivalent gRPC provider** (Shyft, Triton) with matching throughput.

At that point the filter is disabled (`active = TRUE` for all rows, or filter
bypassed entirely in the dispatcher), and ingestion returns to full
protocol-centric coverage. The `watched_pools` table stays in the schema — it
becomes purely informational rather than enforced.

---

## See also

- [`crates/README.md`](../README.md#persistence-yog-persistence) — crate architecture, repository pattern, sqlx offline cache, database roles
- [`migrations/README.md`](./migrations/README.md) — migration conventions (forward-only, GRANTs per migration, local workflow)
- [Root README](../../README.md) — project pitch, roadmap, getting started