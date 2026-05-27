/**
 * High-level fetcher for `GET /api/pools/{address}/swaps`.
 *
 * Paginated feed of swap events for a single pool, ordered most-recent
 * first (`timestamp DESC`, `signature ASC` as tiebreaker).
 *
 * Pagination is cursor-based: the first call passes no cursor (or
 * leaves it absent), subsequent calls pass back the `next_cursor`
 * returned by the previous call. A `null` `next_cursor` indicates the
 * terminal page; a non-null `next_cursor` on a full page may still
 * yield an empty page on the next call (see the contract on `Page`
 * in `crates/core/src/tools/pagination.rs`).
 */

import { apiGet } from "./client";
import { isValidPoolAddress } from "./pool";
import { SwapEventsPageSchema, type SwapEventsPageResponse } from "./schema/page";

/**
 * Bounds mirror yog-api's `MAX_LIMIT` for swap feeds. Kept in sync
 * manually — drift surfaces as an `ApiClientError("http", 400)`.
 */
const DEFAULT_LIMIT = 50;
const MAX_LIMIT = 200;

/**
 * Parameters accepted by `fetchPoolSwaps`. `cursor` defaults to absent
 * (first page); `limit` defaults to 50.
 */
export type FetchPoolSwapsParams = {
  cursor?: string;
  limit?: number;
};

/**
 * Fetch a paginated page of swap events for a pool.
 *
 * @throws TypeError if `address` is syntactically invalid.
 * @throws RangeError if `limit` is outside `[1, MAX_LIMIT]`.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */
export async function fetchPoolSwaps(
  address: string,
  params: FetchPoolSwapsParams = {},
): Promise<SwapEventsPageResponse> {
  if (!isValidPoolAddress(address)) {
    throw new TypeError(`invalid pool address: ${address}`);
  }

  const limit = params.limit ?? DEFAULT_LIMIT;

  if (!Number.isInteger(limit) || limit < 1 || limit > MAX_LIMIT) {
    throw new RangeError(
      `\`limit\` must be an integer in [1, ${MAX_LIMIT}], got ${limit}`,
    );
  }

  return apiGet(
    `/api/pools/${address}/swap-events`,
    {
      cursor:
        params.cursor && params.cursor.length > 0 ? params.cursor : undefined,
      limit,
    },
    SwapEventsPageSchema,
  );
}

/** Re-export the bounds so route handlers can mirror them. */
export const POOL_SWAPS_QUERY_BOUNDS = {
  DEFAULT_LIMIT,
  MAX_LIMIT,
} as const;