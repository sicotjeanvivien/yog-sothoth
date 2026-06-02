/**
 * BFF route handler for `GET /api/pools`.
 *
 * Sits between the browser and `yog-api`. Responsibilities, in order:
 *
 *   1. Parse and validate query parameters (`cursor`, `limit`).
 *   2. Delegate to `fetchPools` for the actual upstream call.
 *   3. Return the typed page on success, or a translated RFC 9457
 *      Problem Details error via `mapApiClientErrorToHttp` on failure.
 *
 * The handler is intentionally thin — every reusable piece of logic
 * (URL building, timeout, schema validation, error classification,
 * Problem Details construction) lives in `src/lib/api/`. Future
 * endpoints follow the same shape.
 *
 * Live under `app/api/pools/route.ts` (not `app/[locale]/api/...`):
 * API resources are locale-agnostic — the browser always fetches the
 * same `/api/pools` regardless of the active locale. Error message
 * localisation happens in React via next-intl, keyed on the typed
 * `title` field returned in the Problem Details body.
 */

import { NextResponse } from "next/server";

import { ApiClientError } from "@/lib/api/errors";
import {
  badRequestProblem,
  internalErrorProblem,
  mapApiClientErrorToHttp,
  problemResponse,
} from "@/lib/api/http-mapping";
import { POOLS_QUERY_BOUNDS, fetchPools, type FetchPoolsParams } from "@/lib/api/pools";

/**
 * Force dynamic execution — the response depends on live data from
 * yog-api, never cache the route itself. Caching strategy lives in
 * `apiGet` (currently `no-store` for every upstream call).
 */
export const dynamic = "force-dynamic";

// ─────────────────────────────────────────────────────────────────────
// Input parsing
// ─────────────────────────────────────────────────────────────────────

/**
 * Result of parsing the public query string.
 *
 * `ok = false` carries a 400-ready message; `ok = true` carries the
 * sanitised parameters ready for `fetchPools`. Keeping this as a
 * discriminated union makes the handler body branch trivially.
 *
 * `cursor` is intentionally absent (not `string | undefined`) when
 * unset — required by `exactOptionalPropertyTypes: true`.
 */
type ParsedQuery =
  | { ok: true; cursor?: string; limit: number }
  | { ok: false; error: string };

function parseQuery(searchParams: URLSearchParams): ParsedQuery {
  const rawLimit = searchParams.get("limit");
  const rawCursor = searchParams.get("cursor");

  // ── limit ──────────────────────────────────────────────────────────
  // Explicit `number` annotation: `POOLS_QUERY_BOUNDS` is `as const`,
  // so the default would be inferred as the literal `50`, refusing
  // any reassignment.
  let limit: number = POOLS_QUERY_BOUNDS.DEFAULT_LIMIT;
  if (rawLimit !== null) {
    const parsed = Number(rawLimit);
    if (!Number.isInteger(parsed)) {
      return {
        ok: false,
        error: `\`limit\` must be an integer, got "${rawLimit}"`,
      };
    }
    if (parsed < 1 || parsed > POOLS_QUERY_BOUNDS.MAX_LIMIT) {
      return {
        ok: false,
        error: `\`limit\` must be between 1 and ${POOLS_QUERY_BOUNDS.MAX_LIMIT}, got ${parsed}`,
      };
    }
    limit = parsed;
  }

  // ── cursor ─────────────────────────────────────────────────────────
  // Opaque from our perspective: yog-api owns the encoding, we forward
  // verbatim. An empty string is treated as absent — same convention
  // as `fetchPools`. With `exactOptionalPropertyTypes`, we omit the
  // key entirely when absent rather than setting it to `undefined`.
  if (rawCursor !== null && rawCursor.length > 0) {
    return { ok: true, cursor: rawCursor, limit };
  }
  return { ok: true, limit };
}

// ─────────────────────────────────────────────────────────────────────
// Handler
// ─────────────────────────────────────────────────────────────────────

export async function GET(request: Request): Promise<NextResponse> {
  const url = new URL(request.url);
  const parsed = parseQuery(url.searchParams);

  if (!parsed.ok) {
    return problemResponse(badRequestProblem(parsed.error));
  }

  try {
    // Build the params object incrementally so the optional `cursor`
    // key is only set when present — required by
    // `exactOptionalPropertyTypes: true` on `FetchPoolsParams`.
    const params: FetchPoolsParams = { limit: parsed.limit };
    if (parsed.cursor !== undefined) {
      params.cursor = parsed.cursor;
    }

    const page = await fetchPools(params);
    return NextResponse.json(page);
  } catch (err) {
    if (err instanceof ApiClientError) {
      // Log the full internal detail server-side; the response body
      // returned to the browser is the sanitised Problem Details.
      console.error("[BFF] /api/pools failed:", err.message, err.details);
      const { status, body } = mapApiClientErrorToHttp(err);
      return problemResponse(body, { status });
    }

    // RangeError from `fetchPools` — should not happen since we
    // validated bounds above, but if it does it's a programmer error
    // in the BFF itself, not a 4xx for the browser.
    console.error("[BFF] /api/pools unexpected error:", err);
    return problemResponse(internalErrorProblem());
  }
}