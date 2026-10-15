/**
 * High-level fetcher for `GET /api/pools/{address}/liquidity-events`.
 *
 * Bidirectional pagination — see `fetchPools` for the full contract.
 * Display order is most-recent first.
 */

import { assertInBounds } from "../client/client-core";
import { apiGet } from "../client/server";
import { isValidPoolAddress } from "./pool";
import {
  zListPoolLiquidityEventsQuery,
  zListPoolLiquidityEventsResponse,
  type PageResponseLiquidityEventResponse,
} from "../schema/zod.gen";
import type { PageDir, PagePosition } from "../type/pagination";

const DEFAULT_LIMIT = 50;

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
 * @throws RangeError if `limit` is outside the contract's bounds.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */
export async function fetchPoolLiquidityEvents(
  address: string,
  params: FetchPoolLiquidityEventsParams = {},
): Promise<PageResponseLiquidityEventResponse> {
  if (!isValidPoolAddress(address)) {
    throw new TypeError(`invalid pool address: ${address}`);
  }

  const limit = params.limit ?? DEFAULT_LIMIT;

  assertInBounds("limit", limit, zListPoolLiquidityEventsQuery.shape.limit.unwrap());

  return apiGet(
    `/api/pools/${address}/liquidity-events`,
    {
      cursor:
        params.cursor && params.cursor.length > 0 ? params.cursor : undefined,
      dir: params.dir,
      position: params.position,
      limit,
    },
    zListPoolLiquidityEventsResponse,
  );
}
