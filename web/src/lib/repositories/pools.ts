import type { Sql } from "postgres";
import { ZodError } from "zod";
import { DatabaseError } from "@/lib/db/errors";
import { poolRowSchema, toPool, type Pool } from "./schemas";

// Repository for the `pools` table.
//
// Repositories own the SQL and the row-to-domain conversion. Route
// Handlers consume the camelCase domain shape and never see raw
// rows or driver errors directly.

/**
 * Fetch every pool observed by the indexer, ordered by most recent
 * activity first. v0.1 has at most a few dozen rows under the
 * `watched_pools` allowlist, so paginating is unnecessary.
 *
 * Failures are surfaced as `DatabaseError`:
 *   - `connection`: the driver could not talk to Postgres
 *   - `query`: SQL-level error (permission denied, unknown column…)
 *   - `validation`: rows came back but a column doesn't match the
 *     expected shape, signalling a schema drift
 */
export async function listPools(sql: Sql): Promise<Pool[]> {
  let rows: unknown[];

  try {
    // postgres.js returns a Result object that is array-like; we
    // copy it into a plain array so downstream code can treat it
    // as a regular `unknown[]` without the driver-specific extras.
    const result = await sql`
      SELECT
        pool_address,
        protocol,
        token_a_mint,
        token_b_mint,
        first_seen_at,
        last_seen_at
      FROM pools
      ORDER BY last_seen_at DESC
    `;
    rows = [...result];
  } catch (cause) {
    throw classifyPostgresError(cause);
  }

  // Validate every row before returning. A schema mismatch here is
  // a genuine bug — the indexer wrote something the API does not
  // know how to interpret — and must not silently degrade output.
  try {
    return rows.map((row) => toPool(poolRowSchema.parse(row)));
  } catch (cause) {
    if (cause instanceof ZodError) {
      throw new DatabaseError(
        "validation",
        "A row in `pools` does not match the expected schema.",
        cause,
      );
    }
    throw new DatabaseError(
      "unknown",
      "Unexpected error while validating pool rows.",
      cause,
    );
  }
}

/**
 * Map a raw error thrown by postgres.js to a domain-level
 * `DatabaseError`. Connection-class errors (ECONNREFUSED, ETIMEDOUT,
 * authentication failures) are surfaced as `connection`; everything
 * else falls back to `query`.
 */
function classifyPostgresError(cause: unknown): DatabaseError {
  // Reuse an already-domain-typed error untouched.
  if (cause instanceof DatabaseError) {
    return cause;
  }

  // postgres.js attaches a `code` to its error objects. The exact
  // catalog matches PostgreSQL's SQLSTATE codes plus a handful of
  // node-side codes for transport-level issues. Treat anything
  // network/auth-shaped as a `connection` failure.
  const code = extractErrorCode(cause);
  const connectionCodes = new Set([
    "ECONNREFUSED",
    "ECONNRESET",
    "ETIMEDOUT",
    "ENOTFOUND",
    "CONNECTION_ENDED",
    "CONNECTION_DESTROYED",
    "CONNECT_TIMEOUT",
    // SQLSTATE 28xxx — invalid authorization (wrong password etc.)
    "28000",
    "28P01",
    // SQLSTATE 08xxx — connection exception
    "08000",
    "08003",
    "08006",
    "08001",
    "08004",
  ]);

  const message = cause instanceof Error ? cause.message : String(cause);

  if (code !== undefined && connectionCodes.has(code)) {
    return new DatabaseError("connection", message, cause);
  }

  return new DatabaseError("query", message, cause);
}

function extractErrorCode(cause: unknown): string | undefined {
  if (cause === null || typeof cause !== "object") {
    return undefined;
  }
  const code = (cause as { code?: unknown }).code;
  return typeof code === "string" ? code : undefined;
}