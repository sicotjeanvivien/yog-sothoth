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

import type * as z from "zod";

import { apiGet } from "../client/server";
import {
  type zListTopPoolsQuery,
  zListTopPoolsResponse,
  type PoolResponse,
} from "../schema/zod.gen";

/** Ranking metric for the top-N pools — the API's `metric` param. */
export type PoolRankMetric = NonNullable<z.infer<typeof zListTopPoolsQuery>["metric"]>;

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
    zListTopPoolsResponse,
  );
}
