/**
 * High-level fetcher for `GET /api/pools`.
 *
 * Validates the input parameters before sending the request — out-of-range
 * `limit` or a malformed `cursor` are caller bugs, not HTTP failures, so
 * we surface them as plain `RangeError` / `TypeError` rather than turning
 * them into `ApiClientError`. The route handler that wraps this function
 * is expected to apply its own input validation against the public query
 * string (it shares the same bounds).
 */

import { apiGet } from "./client";
import { PoolsPageSchema, type PoolsPage } from "./schemas";

/**
 * Defaults mirror the yog-api handler. Kept in sync manually — a
 * schema drift here would show up immediately in the integration
 * tests (Commit 5).
 */
const DEFAULT_LIMIT = 50;
const MAX_LIMIT = 200;

/**
 * Parameters accepted by `fetchPools`. All optional — calling with `{}`
 * returns the first 50 pools ordered by `first_seen_at DESC` (yog-api
 * default ordering).
 */
export type FetchPoolsParams = {
  cursor?: string;
  limit?: number;
};

/**
 * Fetch a paginated page of pools from `yog-api`.
 *
 * @throws RangeError if `limit` is outside `[1, MAX_LIMIT]`.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */
export async function fetchPools(params: FetchPoolsParams = {}): Promise<PoolsPage> {
  const limit = params.limit ?? DEFAULT_LIMIT;

  // Input validation — caller bug, not an HTTP error. Throwing a plain
  // `RangeError` lets the route handler return 400 with a precise
  // client-facing message instead of forwarding a 400 from yog-api.
  if (!Number.isInteger(limit) || limit < 1 || limit > MAX_LIMIT) {
    throw new RangeError(`\`limit\` must be an integer in [1, ${MAX_LIMIT}], got ${limit}`);
  }

  return apiGet(
    "/api/pools",
    {
      // Pass the cursor through as-is when present. An empty string is
      // dropped by the schema-level `default(undefined)` semantics; we
      // turn it into `undefined` here to make the intent explicit.
      cursor: params.cursor && params.cursor.length > 0 ? params.cursor : undefined,
      limit,
    },
    PoolsPageSchema,
  );
}

/** Re-export the bounds so route handlers can mirror them in their own input validation. */
export const POOLS_QUERY_BOUNDS = {
  DEFAULT_LIMIT,
  MAX_LIMIT,
} as const;