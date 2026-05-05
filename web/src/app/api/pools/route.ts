import { NextResponse } from "next/server";
import { getSql } from "@/lib/db/client";
import { isDatabaseError } from "@/lib/db/errors";
import { listPools } from "@/lib/repositories/pools";

// GET /api/pools — list every pool observed by the indexer.
//
// This route is a Server-side Route Handler, executed in the Node.js
// runtime by Next.js 16. It owns the mapping from domain-level
// failures to HTTP responses; the repository layer remains unaware
// of HTTP semantics.

// Force dynamic execution. The data depends on the live indexer
// state and must never be cached statically — even an early
// optimization-driven cache would mask freshness issues.
export const dynamic = "force-dynamic";

export async function GET(): Promise<NextResponse> {
  try {
    const sql = getSql();
    const pools = await listPools(sql);
    return NextResponse.json({ pools });
  } catch (error) {
    return mapErrorToResponse(error);
  }
}

/**
 * Translate a thrown error into a JSON HTTP response.
 *
 * - `connection`     → 503 Service Unavailable (the database is
 *                      unreachable, the rest of the app may still
 *                      work fine; clients can retry)
 * - `query`          → 500 Internal Server Error (the SQL itself
 *                      misfired, this is a bug on our side)
 * - `validation`     → 500 Internal Server Error (schema drift
 *                      between the indexer and the web app)
 * - anything else    → 500 Internal Server Error
 *
 * The body intentionally exposes only a high-level reason string;
 * full error details are kept server-side and logged.
 */
function mapErrorToResponse(error: unknown): NextResponse {
  if (isDatabaseError(error)) {
    console.error(`[/api/pools] DatabaseError(${error.kind}):`, error);

    if (error.kind === "connection") {
      return NextResponse.json(
        { error: "database_unavailable" },
        { status: 503 },
      );
    }
    return NextResponse.json(
      { error: "internal_error" },
      { status: 500 },
    );
  }

  console.error("[/api/pools] Unexpected error:", error);
  return NextResponse.json(
    { error: "internal_error" },
    { status: 500 },
  );
}