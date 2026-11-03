/**
 * The first page of the signals on the server, every protocol's or one protocol's, the live
 * stream following.
 */

import { assertInBounds } from "../client/client-core";
import { signalsPath } from "@/lib/signals/signal-paths";

import { apiGet } from "../client/server";
import {
  zListSignalsQuery,
  zListSignalsResponse,
  type PageResponseSignalResponse,
  type ProtocolResponse,
} from "../schema/zod.gen";

const DEFAULT_LIMIT = 50;

export type FetchSignalsParams = {
  limit?: number;
  protocol?: ProtocolResponse;
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

  return apiGet(signalsPath(params.protocol), { limit }, zListSignalsResponse);
}
