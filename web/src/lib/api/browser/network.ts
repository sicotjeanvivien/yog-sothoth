/**
 * Browser-side fetcher for `GET /api/network`: the slot and latency the
 * dashboard's live indicator polls through the public gateway.
 *
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import { apiGetBrowser } from "@/lib/api/client/browser";
import { zGetNetworkResponse, type NetworkResponse } from "../schema/zod.gen";

export async function fetchNetworkBrowser(): Promise<NetworkResponse> {
  return apiGetBrowser("/api/network", {}, zGetNetworkResponse);
}
