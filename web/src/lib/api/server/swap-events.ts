/** `GET /api/pools/{address}/swap-events`, newest first, paginated as `fetchPools`. */

import { assertInBounds } from "../client/client-core";
import { apiGet } from "../client/server";
import { isValidPoolAddress } from "../server/pool";
import {
  zListPoolSwapsQuery,
  zListPoolSwapsResponse,
  type PageResponseSwapEventResponse,
} from "../schema/zod.gen";
import type { PageDir, PagePosition } from "../type/pagination";

const DEFAULT_LIMIT = 50;

export type FetchPoolSwapEventsParams = {
  cursor?: string | undefined;
  dir?: PageDir | undefined;
  position?: PagePosition | undefined;
  limit?: number;
};

/**
 * @throws TypeError if `address` is syntactically invalid.
 * @throws RangeError if `limit` is outside the contract's bounds.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */
export async function fetchPoolSwapEvents(
  address: string,
  params: FetchPoolSwapEventsParams = {},
): Promise<PageResponseSwapEventResponse> {
  if (!isValidPoolAddress(address)) {
    throw new TypeError(`invalid pool address: ${address}`);
  }

  const limit = params.limit ?? DEFAULT_LIMIT;

  assertInBounds("limit", limit, zListPoolSwapsQuery.shape.limit.unwrap());

  return apiGet(
    `/api/pools/${address}/swap-events`,
    {
      cursor:
        params.cursor && params.cursor.length > 0 ? params.cursor : undefined,
      dir: params.dir,
      position: params.position,
      limit,
    },
    zListPoolSwapsResponse,
  );
}
