/**
 * BFF route handler for `GET /api/pools/{address}/liquidity-events`.
 *
 * Mirror of the `swaps` handler — same parsing, same error mapping,
 * different fetcher. The two stay structurally identical so any future
 * cross-cutting change (auth headers, request ID propagation, rate
 * limiting) lands in both with the same shape.
 */

import { NextResponse } from "next/server";

import { ApiClientError } from "@/lib/api/errors";
import { mapApiClientErrorToHttp } from "@/lib/api/http-mapping";
import {
  POOL_LIQUIDITY_EVENTS_QUERY_BOUNDS,
  fetchPoolLiquidityEvents,
  type FetchPoolLiquidityEventsParams,
} from "@/lib/api/liquidity-events";
import { isValidPoolAddress } from "@/lib/api/pool";

export const dynamic = "force-dynamic";

type RouteParams = { params: Promise<{ address: string }> };

// ─────────────────────────────────────────────────────────────────────
// Input parsing — same shape as the other paginated handlers
// ─────────────────────────────────────────────────────────────────────

type ParsedQuery =
  | { ok: true; cursor?: string; limit: number }
  | { ok: false; error: string };

function parseQuery(searchParams: URLSearchParams): ParsedQuery {
  const rawLimit = searchParams.get("limit");
  const rawCursor = searchParams.get("cursor");

  let limit: number = POOL_LIQUIDITY_EVENTS_QUERY_BOUNDS.DEFAULT_LIMIT;
  if (rawLimit !== null) {
    const parsed = Number(rawLimit);
    if (!Number.isInteger(parsed)) {
      return {
        ok: false,
        error: `\`limit\` must be an integer, got "${rawLimit}"`,
      };
    }
    if (parsed < 1 || parsed > POOL_LIQUIDITY_EVENTS_QUERY_BOUNDS.MAX_LIMIT) {
      return {
        ok: false,
        error: `\`limit\` must be between 1 and ${POOL_LIQUIDITY_EVENTS_QUERY_BOUNDS.MAX_LIMIT}, got ${parsed}`,
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
    return NextResponse.json(
      { error: `invalid pool address: ${address}`, kind: "bad_request" as const },
      { status: 400 },
    );
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
    const fetchParams: FetchPoolLiquidityEventsParams = { limit: parsed.limit };
    if (parsed.cursor !== undefined) {
      fetchParams.cursor = parsed.cursor;
    }

    const page = await fetchPoolLiquidityEvents(address, fetchParams);
    return NextResponse.json(page);
  } catch (err) {
    if (err instanceof ApiClientError) {
      console.error(
        `[BFF] /api/pools/${address}/liquidity-events failed:`,
        err.message,
        err.details,
      );
      const { status, body } = mapApiClientErrorToHttp(err);
      return NextResponse.json(body, { status });
    }

    console.error(
      `[BFF] /api/pools/${address}/liquidity-events unexpected error:`,
      err,
    );
    return NextResponse.json(
      { error: "internal server error", kind: "bad_gateway" as const },
      { status: 500 },
    );
  }
}