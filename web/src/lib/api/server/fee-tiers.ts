/**
 * `GET /api/pools/fee-tiers`: the most common tiers, ascending, with their pool counts. The
 * raw `feeBps` string is replayed as `fee_bps`.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import { apiGet } from "../client/server";
import { zListFeeTiersResponse, type FeeTierResponse } from "../schema/zod.gen";

export async function fetchFeeTiers(): Promise<FeeTierResponse[]> {
  return apiGet("/api/pools/fee-tiers", {}, zListFeeTiersResponse);
}
