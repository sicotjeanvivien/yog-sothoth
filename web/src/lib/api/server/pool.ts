/**
 * `GET /api/pools/{address}`: a pool never observed is a 404 `ApiClientError`, a malformed
 * address a `TypeError`.
 */

import { apiGet } from "../client/server";
import { isValidPoolAddress } from "../pool-address";
import { zGetPoolResponse, type PoolDetailResponse } from "../schema/zod.gen";

// Lives in the runtime-neutral `../pool-address`, so browser fetchers need not import this.
export { isValidPoolAddress };

/**
 * @throws TypeError if `address` is syntactically invalid.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */
export async function fetchPool(address: string): Promise<PoolDetailResponse> {
  if (!isValidPoolAddress(address)) {
    throw new TypeError(`invalid pool address: ${address}`);
  }

  return apiGet(`/api/pools/${address}`, {}, zGetPoolResponse);
}