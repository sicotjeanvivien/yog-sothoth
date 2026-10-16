/**
 * `GET /api/signals` from the browser, to refill the gap after an SSE reconnection.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import { apiGetBrowser } from "@/lib/api/client/browser";
import { zListSignalsResponse, type PageResponseSignalResponse } from "../schema/zod.gen";

const DEFAULT_LIMIT = 50;

export async function fetchSignalsBrowser(
  limit: number = DEFAULT_LIMIT,
): Promise<PageResponseSignalResponse> {
  return apiGetBrowser("/api/signals", { limit }, zListSignalsResponse);
}
