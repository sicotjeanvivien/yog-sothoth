/**
 * High-level fetcher for `GET /api/pools/{address}`.
 *
 * Returns the identity and discovery metadata of a single pool, or
 * throws `ApiClientError` with `kind: "http"` and `status: 404` if the
 * pool has never been observed. The route handler that wraps this
 * function translates that into a 404 for the browser via
 * `mapApiClientErrorToHttp`.
 *
 * Input validation: a malformed `address` (not a base58 pubkey shape)
 * is a caller bug rather than an HTTP failure, so we surface it as a
 * `TypeError`. The BFF route handler applies the same check against
 * the public path parameter before calling this function.
 */

import { apiGet } from "../client/server";
import { isValidPoolAddress } from "../pool-address";
import { PoolSchema, type PoolResponse } from "../schema/pool";

// Re-exported for the existing server fetchers that import it from here; the
// implementation now lives in the runtime-neutral `../pool-address` so the
// browser fetchers can share it without importing this (server) module.
export { isValidPoolAddress };

/**
 * Fetch a single pool by its on-chain address.
 *
 * @throws TypeError if `address` is syntactically invalid.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 *         A 404 from yog-api surfaces as `kind: "http", status: 404`.
 */
export async function fetchPool(address: string): Promise<PoolResponse> {
  if (!isValidPoolAddress(address)) {
    throw new TypeError(`invalid pool address: ${address}`);
  }

  return apiGet(`/api/pools/${address}`, {}, PoolSchema);
}