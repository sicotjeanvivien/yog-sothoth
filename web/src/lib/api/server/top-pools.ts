/**
 * `GET /api/pools/top`: the pools ranked by flow (`volume_24h`, the default) or depth (`tvl`),
 * as the list's `PoolResponse`.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import type * as z from "zod";

import { apiGet } from "../client/server";
import {
  type zListTopPoolsQuery,
  zListTopPoolsResponse,
  type PoolResponse,
} from "../schema/zod.gen";

export type PoolRankMetric = NonNullable<z.infer<typeof zListTopPoolsQuery>["metric"]>;

/** The default metric is sent by omitting the param. */
export async function fetchTopPools(
  metric: PoolRankMetric = "volume_24h",
): Promise<PoolResponse[]> {
  return apiGet(
    "/api/pools/top",
    { metric: metric === "volume_24h" ? undefined : metric },
    zListTopPoolsResponse,
  );
}
