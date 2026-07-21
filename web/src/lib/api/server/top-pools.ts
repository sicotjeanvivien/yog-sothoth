/**
 * High-level fetcher for `GET /api/pools/top`.
 *
 * Returns the top pools ranked by `metric`, as a plain (non-paginated) array
 * of the same enriched `PoolResponse` the list endpoint emits — so the UI
 * reuses the pool schema and the pair cell. The two metrics are different
 * lenses: `volume_24h` is flow (actively traded), `tvl` is depth (liquidity
 * parked). The server defaults the metric to `volume_24h` and `limit` to 10.
 *
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import * as z from "zod";

import { apiGet } from "../client/server";
import { PoolSchema, type PoolResponse } from "../schema/pool";

/** Ranking metric for the top-N pools — mirrors the API's `metric` param. */
export type PoolRankMetric = "volume_24h" | "tvl";

const TopPoolsSchema = z.array(PoolSchema);

/**
 * Fetch the top pools ranked by `metric` (default `volume_24h`) from `yog-api`.
 * The default is sent implicitly (omitted param) so an unfiltered call keeps
 * the same URL as before.
 */
export async function fetchTopPools(
  metric: PoolRankMetric = "volume_24h",
): Promise<PoolResponse[]> {
  return apiGet(
    "/api/pools/top",
    { metric: metric === "volume_24h" ? undefined : metric },
    TopPoolsSchema,
  );
}
