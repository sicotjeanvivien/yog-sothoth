/**
 * Browser-side fetcher for `GET /api/ingestion`: the freshness the
 * sidebar's `NetworkStatusPanel` polls through the public gateway.
 *
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import { apiGetBrowser } from "@/lib/api/client/browser";
import { zGetIngestionResponse, type IngestionResponse } from "../schema/zod.gen";

export async function fetchIngestionBrowser(): Promise<IngestionResponse> {
  return apiGetBrowser("/api/ingestion", {}, zGetIngestionResponse);
}
