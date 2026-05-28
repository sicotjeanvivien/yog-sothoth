/**
 * High-level fetcher for `GET /api/pools`.
 *
 * Bidirectional pagination: pass `cursor` + `dir` to traverse the
 * list, or `position` to jump to a boundary. `cursor` and `position`
 * are mutually exclusive — yog-api will return a 400 if both are
 * sent, but we don't validate that here: it's the URL state's job to
 * keep them consistent.
 *
 * Validates the local input parameters before sending the request —
 * out-of-range `limit` is a caller bug, not an HTTP failure, so we
 * surface it as `RangeError` rather than turning it into an
 * `ApiClientError`.
 */

import { apiGet } from "./client";
import { PoolsPageSchema, type PoolsPageResponse } from "./schema/page";
import type { PageDir, PagePosition } from "./type/pagination";

const DEFAULT_LIMIT = 50;
const MAX_LIMIT = 200;

/**
 * Parameters accepted by `fetchPools`. All optional.
 *
 * - `{}` returns the first page (newest pools).
 * - `{ cursor, dir }` paginates relative to the cursor.
 * - `{ position: "last" }` jumps to the oldest pools.
 *
 * `cursor` + `position` should not be combined; the API will
 * reject the request with 400 if they are.
 */
export type FetchPoolsParams = {
  cursor?: string | undefined;
  dir?: PageDir | undefined;
  position?: PagePosition | undefined;
  q?: string | undefined;
  limit?: number;
};

/**
 * Fetch a paginated page of pools from `yog-api`.
 *
 * @throws RangeError if `limit` is outside `[1, MAX_LIMIT]`.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */
export async function fetchPools(
  params: FetchPoolsParams = {},
): Promise<PoolsPageResponse> {
  const limit = params.limit ?? DEFAULT_LIMIT;

  if (!Number.isInteger(limit) || limit < 1 || limit > MAX_LIMIT) {
    throw new RangeError(
      `\`limit\` must be an integer in [1, ${MAX_LIMIT}], got ${limit}`,
    );
  }

  return apiGet(
    "/api/pools",
    {
      cursor:
        params.cursor && params.cursor.length > 0 ? params.cursor : undefined,
      dir: params.dir,
      position: params.position,
      q: params.q && params.q.length > 0 ? params.q : undefined,
      limit,
    },
    PoolsPageSchema,
  );
}

export const POOLS_QUERY_BOUNDS = {
  DEFAULT_LIMIT,
  MAX_LIMIT,
} as const;