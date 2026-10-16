/** `GET /api/pools/{address}/history`: hourly USD activity over `days`, oldest first. */

import { assertInBounds } from "../client/client-core";
import { apiGet } from "../client/server";
import { isValidPoolAddress } from "../server/pool";
import {
  zGetPoolHistoryQuery,
  zGetPoolHistoryResponse,
  type PoolHistoryBucketResponse,
} from "../schema/zod.gen";

const DEFAULT_DAYS = 7;

/**
 * @throws TypeError if `address` is syntactically invalid.
 * @throws RangeError if `days` is outside the contract's bounds.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */
export async function fetchPoolHistory(
  address: string,
  days: number = DEFAULT_DAYS,
): Promise<PoolHistoryBucketResponse[]> {
  if (!isValidPoolAddress(address)) {
    throw new TypeError(`invalid pool address: ${address}`);
  }
  assertInBounds("days", days, zGetPoolHistoryQuery.shape.days.unwrap());

  return apiGet(`/api/pools/${address}/history`, { days }, zGetPoolHistoryResponse);
}
