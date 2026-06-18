/**
 * High-level fetcher for `GET /api/stats`.
 *
 * Like `fetchNetworkStatus`, this endpoint takes no parameters — a thin
 * wrapper over `apiGet` that pins the path and the schema.
 *
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import { apiGet } from "../client/server";
import { StatsSchema, type StatsResponse } from "../schema/stats";

/** Fetch the current protocol-wide statistics snapshot from `yog-api`. */
export async function fetchStats(): Promise<StatsResponse> {
  // No query parameters — the empty object keeps the `apiGet` signature satisfied.
  return apiGet("/api/stats", {}, StatsSchema);
}
