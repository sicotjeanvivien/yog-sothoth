/**
 * High-level fetcher for `GET /api/pools/{address}/liquidity-events`.
 *
 * Paginated feed of liquidity events (add / remove) for a single pool,
 * ordered most-recent first (`timestamp DESC`, `signature ASC` as
 * tiebreaker). Same pagination contract as `fetchPoolSwaps`.
 */

import { apiGet } from "./client";
import { isValidPoolAddress } from "./pool";
import {
  LiquidityEventsPageSchema,
  type LiquidityEventsPage,
} from "./schemas";

const DEFAULT_LIMIT = 50;
const MAX_LIMIT = 200;

export type FetchPoolLiquidityEventsParams = {
  cursor?: string;
  limit?: number;
};

/**
 * Fetch a paginated page of liquidity events for a pool.
 *
 * @throws TypeError if `address` is syntactically invalid.
 * @throws RangeError if `limit` is outside `[1, MAX_LIMIT]`.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */
export async function fetchPoolLiquidityEvents(
  address: string,
  params: FetchPoolLiquidityEventsParams = {},
): Promise<LiquidityEventsPage> {
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
    `/api/pools/${address}/liquidity-events`,
    {
      cursor:
        params.cursor && params.cursor.length > 0 ? params.cursor : undefined,
      limit,
    },
    LiquidityEventsPageSchema,
  );
}

export const POOL_LIQUIDITY_EVENTS_QUERY_BOUNDS = {
  DEFAULT_LIMIT,
  MAX_LIMIT,
} as const;