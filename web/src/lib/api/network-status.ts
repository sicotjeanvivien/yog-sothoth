/**
 * High-level fetcher for `GET /api/network/status`.
 *
 * Unlike `fetchPools`, this endpoint takes no parameters — there is
 * nothing to validate before the call, so the function is a thin
 * wrapper over `apiGet` that pins the path and the schema.
 *
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import { apiGet } from "./client";
import { NetworkStatusSchema, type NetworkStatus } from "./schema/network-status";

/** Fetch the current network status snapshot from `yog-api`. */
export async function fetchNetworkStatus(): Promise<NetworkStatus> {
  // No query parameters — the empty object keeps the `apiGet`
  // signature satisfied.
  return apiGet("/api/network/status", {}, NetworkStatusSchema);
}