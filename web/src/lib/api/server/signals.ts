/**
 * `GET /api/signals` on the server: the first page of the global feed, the SSE tail following,
 * or a pool's paginated slice (`pool`).
 */

import { assertInBounds } from "../client/client-core";
import { apiGet } from "../client/server";
import { isValidPoolAddress } from "../pool-address";
import {
  zListSignalsQuery,
  zListSignalsResponse,
  type PageResponseSignalResponse,
} from "../schema/zod.gen";
import type { PageDir, PagePosition } from "../type/pagination";

const DEFAULT_LIMIT = 50;

export type FetchSignalsParams = {
  /** Restrict the feed to one pool's signals (base58 address). */
  pool?: string | undefined;
  cursor?: string | undefined;
  dir?: PageDir | undefined;
  position?: PagePosition | undefined;
  limit?: number;
};

/**
 * @throws TypeError if `pool` is set but syntactically invalid.
 * @throws RangeError if `limit` is outside the contract's bounds.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */
export async function fetchSignals(
  params: FetchSignalsParams = {},
): Promise<PageResponseSignalResponse> {
  if (params.pool !== undefined && !isValidPoolAddress(params.pool)) {
    throw new TypeError(`invalid pool address: ${params.pool}`);
  }

  const limit = params.limit ?? DEFAULT_LIMIT;

  assertInBounds("limit", limit, zListSignalsQuery.shape.limit.unwrap());

  return apiGet(
    "/api/signals",
    {
      pool: params.pool,
      cursor:
        params.cursor && params.cursor.length > 0 ? params.cursor : undefined,
      dir: params.dir,
      position: params.position,
      limit,
    },
    zListSignalsResponse,
  );
}
