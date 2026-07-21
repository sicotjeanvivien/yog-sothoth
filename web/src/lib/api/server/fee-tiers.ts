/**
 * High-level fetcher for `GET /api/pools/fee-tiers`.
 *
 * Returns the distinct base-fee tiers observed across all pools, ascending,
 * as decimal strings in basis points (e.g. `["2.5", "25", "100"]`) — the same
 * precision-safe `BigDecimal` string form as a pool's `feeBps`. Powers the
 * option list of the pools fee filter; the UI formats each tier to a percent
 * for display and replays the raw string as the `fee_bps` query param.
 *
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import * as z from "zod";

import { apiGet } from "../client/server";
import { BigDecimal } from "../schema/shared";

const FeeTiersSchema = z.array(BigDecimal);
export type FeeTiers = z.infer<typeof FeeTiersSchema>;

/** Fetch the observed fee tiers (basis points) from `yog-api`. */
export async function fetchFeeTiers(): Promise<FeeTiers> {
  return apiGet("/api/pools/fee-tiers", {}, FeeTiersSchema);
}
