/**
 * Browser-side fetcher for `GET /api/signals`.
 *
 * Mirrors `lib/api/server/signals.ts` but reaches yog-api through the
 * public gateway (`NEXT_PUBLIC_YOG_API_URL`). Used by the signal feed's
 * stream hook to refill the gap after an SSE reconnection — the stream
 * only carries signals born after it (re)opened.
 *
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import { apiGetBrowser } from "@/lib/api/client/browser";
import { zListSignalsResponse, type PageResponseSignalResponse } from "../schema/zod.gen";

const DEFAULT_LIMIT = 50;

/** Fetch the newest signals (first page of the feed) from the browser. */
export async function fetchSignalsBrowser(
  limit: number = DEFAULT_LIMIT,
): Promise<PageResponseSignalResponse> {
  return apiGetBrowser("/api/signals", { limit }, zListSignalsResponse);
}
