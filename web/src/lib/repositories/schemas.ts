import { z } from "zod";

// Schemas for the `pools` table.
//
// Two schemas are exposed:
//   - `poolRowSchema`: shape of a single row as it comes back from
//     Postgres. Field names match the SQL columns (snake_case).
//   - `poolSchema`: shape exposed by the API. Field names follow
//     TypeScript conventions (camelCase). Timestamps are serialized
//     as ISO strings so they survive a JSON round-trip cleanly.

/**
 * Set of protocol identifiers known to the indexer. Mirrors the
 * `Protocol` enum on the Rust side. Kept open with `z.enum` so that
 * any unexpected value coming from the database fails parsing
 * loudly rather than silently flowing through.
 */
export const protocolSchema = z.enum(["damm_v2", "damm_v1", "dlmm"]);

export type Protocol = z.infer<typeof protocolSchema>;

/**
 * Shape of a row in the `pools` table as returned by postgres.js.
 *
 * postgres.js returns Postgres `TIMESTAMPTZ` columns as JS `Date`
 * objects by default, hence `z.date()` on the timestamp fields.
 */
export const poolRowSchema = z.object({
  pool_address: z.string().min(1),
  protocol: protocolSchema,
  token_a_mint: z.string().min(1),
  token_b_mint: z.string().min(1),
  first_seen_at: z.date(),
  last_seen_at: z.date(),
});

export type PoolRow = z.infer<typeof poolRowSchema>;

/**
 * Shape of a pool as exposed by the HTTP API. Field names are
 * camelCase to match the surrounding TypeScript codebase, and
 * timestamps are pre-serialized to ISO 8601 strings so the response
 * body is plain JSON without any Date/serialization gotchas.
 */
export const poolSchema = z.object({
  poolAddress: z.string(),
  protocol: protocolSchema,
  tokenAMint: z.string(),
  tokenBMint: z.string(),
  firstSeenAt: z.string().datetime(),
  lastSeenAt: z.string().datetime(),
});

export type Pool = z.infer<typeof poolSchema>;

/**
 * Map a validated DB row to its API representation.
 *
 * Centralizing the conversion here keeps the snake_case ↔ camelCase
 * translation in a single, testable function instead of leaking
 * across repositories and route handlers.
 */
export function toPool(row: PoolRow): Pool {
  return {
    poolAddress: row.pool_address,
    protocol: row.protocol,
    tokenAMint: row.token_a_mint,
    tokenBMint: row.token_b_mint,
    firstSeenAt: row.first_seen_at.toISOString(),
    lastSeenAt: row.last_seen_at.toISOString(),
  };
}