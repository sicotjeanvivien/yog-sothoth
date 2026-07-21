/**
 * High-level fetcher for `GET /api/pools/fee-tiers`.
 *
 * Returns the *most common* base-fee tiers, each with its pool count, ascending
 * by fee — the option list of the pools fee filter. The API caps the list (the
 * observed fee distribution is long-tailed) so the dropdown stays short and
 * useful; the count lets the UI label each option (`0.25% · 166`).
 *
 * `feeBps` is a precision-safe `BigDecimal` string in basis points (same form
 * as a pool's `feeBps`); the UI formats it to a percent and replays the raw
 * string as the `fee_bps` query param.
 *
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import * as z from "zod";

import { apiGet } from "../client/server";
import { BigDecimal } from "../schema/shared";

export const FeeTierSchema = z.object({
  feeBps: BigDecimal,
  poolCount: z.number().int().nonnegative(),
});

export type FeeTier = z.infer<typeof FeeTierSchema>;

const FeeTiersSchema = z.array(FeeTierSchema);
export type FeeTiers = z.infer<typeof FeeTiersSchema>;

/** Fetch the observed fee tiers (basis points, with counts) from `yog-api`. */
export async function fetchFeeTiers(): Promise<FeeTiers> {
  return apiGet("/api/pools/fee-tiers", {}, FeeTiersSchema);
}
