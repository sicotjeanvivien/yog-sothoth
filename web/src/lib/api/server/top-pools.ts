/**
 * High-level fetcher for `GET /api/pools/top`.
 *
 * Returns the top pools ranked by the metric, as a plain (non-paginated)
 * array of the same enriched `PoolResponse` the list endpoint emits — so the
 * UI reuses the pool schema and the pair cell. The server defaults the metric
 * to `volume_24h` and `limit` to 10; no params needed for the phase-1 strip.
 *
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import * as z from "zod";

import { apiGet } from "../client/server";
import { PoolSchema, type PoolResponse } from "../schema/pool";

const TopPoolsSchema = z.array(PoolSchema);

/** Fetch the top pools by 24h volume (default ranking) from `yog-api`. */
export async function fetchTopPools(): Promise<PoolResponse[]> {
  return apiGet("/api/pools/top", {}, TopPoolsSchema);
}
