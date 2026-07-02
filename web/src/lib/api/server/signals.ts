/**
 * High-level fetcher for `GET /api/signals` (server runtime).
 *
 * The signal feed page only ever needs the first page server-side (the
 * live tail arrives over SSE afterwards), so this exposes the page-1
 * read: newest signals first, no cursor. Pagination params can be added
 * the day a "load more" ships (the API already supports them).
 */

import { apiGet } from "../client/server";
import { SignalsPageSchema, type SignalsPageResponse } from "../schema/page";

const DEFAULT_LIMIT = 50;
const MAX_LIMIT = 200;

/**
 * Fetch the newest signals (first page of the feed).
 *
 * @throws RangeError if `limit` is outside `[1, MAX_LIMIT]`.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */
export async function fetchSignals(
  limit: number = DEFAULT_LIMIT,
): Promise<SignalsPageResponse> {
  if (!Number.isInteger(limit) || limit < 1 || limit > MAX_LIMIT) {
    throw new RangeError(
      `\`limit\` must be an integer in [1, ${MAX_LIMIT}], got ${limit}`,
    );
  }

  return apiGet("/api/signals", { limit }, SignalsPageSchema);
}

export const SIGNALS_QUERY_BOUNDS = {
  DEFAULT_LIMIT,
  MAX_LIMIT,
} as const;
