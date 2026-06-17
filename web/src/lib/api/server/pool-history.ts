/**
 * High-level fetcher for `GET /api/pools/{address}/history`.
 *
 * Returns an hourly activity time-series (volume, fees, liquidity, claims —
 * all USD) for a pool over the last `days` days, ordered oldest → newest.
 * Not paginated: the window is bounded by `days`.
 */

import { apiGet } from "../client/server";
import { isValidPoolAddress } from "../server/pool";
import { PoolHistorySchema, type PoolHistoryResponse } from "../schema/pool-history";

const DEFAULT_DAYS = 7;
const MAX_DAYS = 90;

/**
 * Fetch the hourly history for a pool.
 *
 * @throws TypeError if `address` is syntactically invalid.
 * @throws RangeError if `days` is outside `[1, MAX_DAYS]`.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */
export async function fetchPoolHistory(
  address: string,
  days: number = DEFAULT_DAYS,
): Promise<PoolHistoryResponse> {
  if (!isValidPoolAddress(address)) {
    throw new TypeError(`invalid pool address: ${address}`);
  }
  if (!Number.isInteger(days) || days < 1 || days > MAX_DAYS) {
    throw new RangeError(`\`days\` must be an integer in [1, ${MAX_DAYS}], got ${days}`);
  }

  return apiGet(`/api/pools/${address}/history`, { days }, PoolHistorySchema);
}

export const POOL_HISTORY_QUERY_BOUNDS = { DEFAULT_DAYS, MAX_DAYS } as const;
