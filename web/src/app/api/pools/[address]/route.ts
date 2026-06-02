/**
 * BFF route handler for `GET /api/pools/{address}`.
 *
 * Validates the `address` path parameter as a base58 pubkey shape,
 * then delegates to `fetchPool`. A 404 from yog-api passes through
 * unchanged via `mapApiClientErrorToHttp` (the `http` variant of
 * `ApiClientError` carries the upstream status).
 */

import { NextResponse } from "next/server";

import { ApiClientError } from "@/lib/api/errors";
import { badRequestProblem, internalErrorProblem, mapApiClientErrorToHttp, problemResponse } from "@/lib/api/http-mapping";
import { fetchPool, isValidPoolAddress } from "@/lib/api/pool";

export const dynamic = "force-dynamic";

/**
 * Next.js 15+ passes route segment params as a Promise. Awaiting it
 * keeps the route handler async-correct and matches the framework's
 * expected signature.
 */
type RouteParams = { params: Promise<{ address: string }> };

export async function GET(
  _request: Request,
  { params }: RouteParams,
): Promise<NextResponse> {
  const { address } = await params;

  if (!isValidPoolAddress(address)) {
    return problemResponse(badRequestProblem(`invalid pool address: ${address}`));
  }

  try {
    const pool = await fetchPool(address);
    return NextResponse.json(pool);
  } catch (err) {
    if (err instanceof ApiClientError) {
      console.error(
        `[BFF] /api/pools/${address} failed:`,
        err.message,
        err.details,
      );
      const { status, body } = mapApiClientErrorToHttp(err);
      return problemResponse(body, { status });
    }
    console.error(`[BFF] /api/pools/${address} unexpected error:`, err);
    return problemResponse(internalErrorProblem());
  }
}