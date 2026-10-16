/**
 * `GET /api/pools/{address}/latest-state`. ⚠️ A 404 does not mean an unknown pool: a pool seen
 * only through claim events has no current state.
 */

import { apiGet } from "../client/server";
import { isValidPoolAddress } from "./pool";
import {
  zGetPoolLatestStateResponse,
  type PoolCurrentStateResponse,
} from "../schema/zod.gen";

/**
 * @throws TypeError if `address` is syntactically invalid.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */
export async function fetchPoolLatestState(
  address: string,
): Promise<PoolCurrentStateResponse> {
  if (!isValidPoolAddress(address)) {
    throw new TypeError(`invalid pool address: ${address}`);
  }

  return apiGet(
    `/api/pools/${address}/latest-state`,
    {},
    zGetPoolLatestStateResponse,
  );
}