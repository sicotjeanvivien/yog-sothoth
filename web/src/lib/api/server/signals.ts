/**
 * `GET /api/signals` on the server: the first page of the global feed, the SSE tail following.
 */

import { assertInBounds } from "../client/client-core";
import { apiGet } from "../client/server";
import {
  zListSignalsQuery,
  zListSignalsResponse,
  type PageResponseSignalResponse,
} from "../schema/zod.gen";

const DEFAULT_LIMIT = 50;

export type FetchSignalsParams = {
  limit?: number;
};

/**
 * @throws RangeError if `limit` is outside the contract's bounds.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */
export async function fetchSignals(
  params: FetchSignalsParams = {},
): Promise<PageResponseSignalResponse> {
  const limit = params.limit ?? DEFAULT_LIMIT;

  assertInBounds("limit", limit, zListSignalsQuery.shape.limit.unwrap());

  return apiGet("/api/signals", { limit }, zListSignalsResponse);
}
