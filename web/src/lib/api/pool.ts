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

import { apiGet } from "./client";
import { PoolResponseSchema, type PoolResponse } from "./schemas";

/**
 * Validate that the address looks like a Solana base58 pubkey.
 *
 * Base58 encodes 32 bytes into 43-44 characters from the Bitcoin
 * alphabet (excludes `0`, `O`, `I`, `l`). We perform a syntactic
 * check only — verifying the bytes round-trip into a valid Pubkey is
 * yog-api's job; this guard exists to reject obviously-wrong input
 * (empty string, URL injection, etc.) before going over the wire.
 */
const BASE58_PUBKEY = /^[1-9A-HJ-NP-Za-km-z]{32,44}$/;

export function isValidPoolAddress(address: string): boolean {
  return BASE58_PUBKEY.test(address);
}

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

  return apiGet(`/api/pools/${address}`, {}, PoolResponseSchema);
}