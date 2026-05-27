/**
 * High-level fetcher for `GET /api/pools/{address}/liquidity-events`.
 *
 * Bidirectional pagination — see `fetchPools` for the full contract.
 * Display order is most-recent first.
 */

import { apiGet } from "./client";
import { isValidPoolAddress } from "./pool";
import {
  LiquidityEventsPageSchema,
  type LiquidityEventsPageResponse,
} from "./schema/page";
import type { PageDir, PagePosition } from "./type/pagination";

const DEFAULT_LIMIT = 50;
const MAX_LIMIT = 200;

export type FetchPoolLiquidityEventsParams = {
  cursor?: string | undefined;
  dir?: PageDir | undefined;
  position?: PagePosition | undefined;
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
): Promise<LiquidityEventsPageResponse> {
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
      dir: params.dir,
      position: params.position,
      limit,
    },
    LiquidityEventsPageSchema,
  );
}

export const POOL_LIQUIDITY_EVENTS_QUERY_BOUNDS = {
  DEFAULT_LIMIT,
  MAX_LIMIT,
} as const;