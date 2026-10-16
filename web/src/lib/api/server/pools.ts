/**
 * `GET /api/pools`. `cursor` + `dir` or `position`, never both: the API answers 400, and the
 * URL state keeps them apart. An out-of-range `limit` is a `RangeError`, a caller bug.
 */

import { assertInBounds } from "../client/client-core";
import { apiGet } from "../client/server";
import {
  zListPoolsQuery,
  zListPoolsResponse,
  type PoolPageResponsePoolResponse,
} from "../schema/zod.gen";
import type { PageDir, PagePosition, PoolSort } from "../type/pagination";

const DEFAULT_LIMIT = 50;

export type FetchPoolsParams = {
  cursor?: string | undefined;
  dir?: PageDir | undefined;
  position?: PagePosition | undefined;
  q?: string | undefined;
  sort?: PoolSort | undefined;
  /** A tier from `fetchFeeTiers`, sent as `fee_bps`. */
  feeBps?: string | undefined;
  limit?: number;
};

/**
 * @throws RangeError if `limit` is outside the contract's bounds.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */
export async function fetchPools(
  params: FetchPoolsParams = {},
): Promise<PoolPageResponsePoolResponse> {
  const limit = params.limit ?? DEFAULT_LIMIT;
  assertInBounds("limit", limit, zListPoolsQuery.shape.limit.unwrap());

  return apiGet(
    "/api/pools",
    {
      cursor:
        params.cursor && params.cursor.length > 0 ? params.cursor : undefined,
      dir: params.dir,
      position: params.position,
      q: params.q && params.q.length > 0 ? params.q : undefined,
      sort: params.sort,
      fee_bps:
        params.feeBps && params.feeBps.length > 0 ? params.feeBps : undefined,
      limit,
    },
    zListPoolsResponse,
  );
}
