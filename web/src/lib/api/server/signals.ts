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

import { apiGet } from "../client/server";
import { isValidPoolAddress } from "../server/pool";
import { SignalsPageSchema, type SignalsPageResponse } from "../schema/page";
import type { PageDir, PagePosition } from "../type/pagination";

const DEFAULT_LIMIT = 50;
const MAX_LIMIT = 200;

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
 * @throws RangeError if `limit` is outside `[1, MAX_LIMIT]`.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */
export async function fetchSignals(
  params: FetchSignalsParams = {},
): Promise<SignalsPageResponse> {
  if (params.pool !== undefined && !isValidPoolAddress(params.pool)) {
    throw new TypeError(`invalid pool address: ${params.pool}`);
  }

  const limit = params.limit ?? DEFAULT_LIMIT;

  if (!Number.isInteger(limit) || limit < 1 || limit > MAX_LIMIT) {
    throw new RangeError(
      `\`limit\` must be an integer in [1, ${MAX_LIMIT}], got ${limit}`,
    );
  }

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
    SignalsPageSchema,
  );
}

export const SIGNALS_QUERY_BOUNDS = {
  DEFAULT_LIMIT,
  MAX_LIMIT,
} as const;
