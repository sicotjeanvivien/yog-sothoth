/**
 * `GET /api/stats`.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import { apiGet } from "../client/server";
import { zGetStatsResponse, type StatsResponse } from "../schema/zod.gen";

export async function fetchStats(): Promise<StatsResponse> {
  return apiGet("/api/stats", {}, zGetStatsResponse);
}
