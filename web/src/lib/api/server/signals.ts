/**
 * High-level fetcher for `GET /api/signals` (server runtime).
 *
 * Two consumers, two shapes of read:
 *   - the `/signals` feed and the Overview block read the first page
 *     of the global feed (no cursor — the live tail arrives over SSE
 *     afterwards);
 *   - the pool-detail "Alerts" tab reads the pool-filtered feed
 *     (`pool` param) with full bidirectional pagination.
 */

import { assertInBounds } from "../client/client-core";
import { apiGet } from "../client/server";
import { isValidPoolAddress } from "../server/pool";
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
 * Fetch a page of the signal feed, newest first.
 *
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
