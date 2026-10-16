/**
 * `GET /api/pools/{address}` from the browser, for the watchlist.
 * @throws TypeError if `address` is syntactically invalid.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import { apiGetBrowser } from "@/lib/api/client/browser";

import { isValidPoolAddress } from "../pool-address";
import { zGetPoolResponse, type PoolDetailResponse } from "../schema/zod.gen";

export async function fetchPoolBrowser(address: string): Promise<PoolDetailResponse> {
  if (!isValidPoolAddress(address)) {
    throw new TypeError(`invalid pool address: ${address}`);
  }

  return apiGetBrowser(`/api/pools/${address}`, {}, zGetPoolResponse);
}
