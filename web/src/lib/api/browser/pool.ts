/**
 * Browser-side fetcher for `GET /api/pools/{address}`.
 *
 * Mirrors `lib/api/server/pool.ts` but reaches yog-api through the public
 * gateway (`NEXT_PUBLIC_YOG_API_URL`). Used by the watchlist page, which reads
 * its pool addresses from LocalStorage (client-only) and fetches each one from
 * the browser — no server round-trip, no batch endpoint needed for the small
 * personal set a watchlist holds.
 *
 * @throws TypeError if `address` is syntactically invalid.
 * @throws ApiClientError on any transport, HTTP, or schema failure. A 404 from
 *         yog-api surfaces as `kind: "http", status: 404`.
 */

import { apiGetBrowser } from "@/lib/api/client/browser";

import { isValidPoolAddress } from "../pool-address";
import { PoolSchema, type PoolResponse } from "../schema/pool";

export async function fetchPoolBrowser(address: string): Promise<PoolResponse> {
  if (!isValidPoolAddress(address)) {
    throw new TypeError(`invalid pool address: ${address}`);
  }

  return apiGetBrowser(`/api/pools/${address}`, {}, PoolSchema);
}
