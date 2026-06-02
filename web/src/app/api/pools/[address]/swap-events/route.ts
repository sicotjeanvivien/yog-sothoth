/**
 * BFF route handler for `GET /api/pools/{address}/swaps`.
 *
 * Validates the path parameter as a base58 pubkey, the query
 * parameters (`cursor`, `limit`), then delegates to `fetchPoolSwapEvents`.
 * Same pattern as `/api/pools`: bounds are shared via
 * `POOL_SWAPS_QUERY_BOUNDS`, transport / upstream failures funnel
 * through `mapApiClientErrorToHttp`.
 */

import { NextResponse } from "next/server";

import { ApiClientError } from "@/lib/api/errors";
import { badRequestProblem, internalErrorProblem, mapApiClientErrorToHttp, problemResponse } from "@/lib/api/http-mapping";
import { isValidPoolAddress } from "@/lib/api/pool";
import {
  POOL_SWAPS_QUERY_BOUNDS,
  fetchPoolSwapEvents,
  type FetchPoolSwapEventsParams,
} from "@/lib/api/swap-events";

export const dynamic = "force-dynamic";

type RouteParams = { params: Promise<{ address: string }> };

// ─────────────────────────────────────────────────────────────────────
// Input parsing — same shape as `app/api/pools/route.ts`
// ─────────────────────────────────────────────────────────────────────

type ParsedQuery =
  | { ok: true; cursor?: string; limit: number }
  | { ok: false; error: string };

function parseQuery(searchParams: URLSearchParams): ParsedQuery {
  const rawLimit = searchParams.get("limit");
  const rawCursor = searchParams.get("cursor");

  let limit: number = POOL_SWAPS_QUERY_BOUNDS.DEFAULT_LIMIT;
  if (rawLimit !== null) {
    const parsed = Number(rawLimit);
    if (!Number.isInteger(parsed)) {
      return {
        ok: false,
        error: `\`limit\` must be an integer, got "${rawLimit}"`,
      };
    }
    if (parsed < 1 || parsed > POOL_SWAPS_QUERY_BOUNDS.MAX_LIMIT) {
      return {
        ok: false,
        error: `\`limit\` must be between 1 and ${POOL_SWAPS_QUERY_BOUNDS.MAX_LIMIT}, got ${parsed}`,
      };
    }
    limit = parsed;
  }

  if (rawCursor !== null && rawCursor.length > 0) {
    return { ok: true, cursor: rawCursor, limit };
  }
  return { ok: true, limit };
}

// ─────────────────────────────────────────────────────────────────────
// Handler
// ─────────────────────────────────────────────────────────────────────

export async function GET(
  request: Request,
  { params }: RouteParams,
): Promise<NextResponse> {
  const { address } = await params;

  if (!isValidPoolAddress(address)) {
    return problemResponse(badRequestProblem(`invalid pool address: ${address}`));
  }

  const url = new URL(request.url);
  const parsed = parseQuery(url.searchParams);

  if (!parsed.ok) {
    return NextResponse.json(
      { error: parsed.error, kind: "bad_request" as const },
      { status: 400 },
    );
  }

  try {
    const fetchParams: FetchPoolSwapEventsParams = { limit: parsed.limit };
    if (parsed.cursor !== undefined) {
      fetchParams.cursor = parsed.cursor;
    }

    const page = await fetchPoolSwapEvents(address, fetchParams);
    return NextResponse.json(page);
  } catch (err) {
    if (err instanceof ApiClientError) {
      console.error(
        `[BFF] /api/pools/${address}/swaps-event failed:`,
        err.message,
        err.details,
      );
      const { status, body } = mapApiClientErrorToHttp(err);
      return problemResponse(body, { status });
    }

    console.error(
      `[BFF] /api/pools/${address}/swaps-event unexpected error:`,
      err,
    );
    return problemResponse(internalErrorProblem());
  }
}