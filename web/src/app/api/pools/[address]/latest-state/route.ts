/**
 * BFF route handler for `GET /api/pools/{address}/latest-state`.
 *
 * Returns the projected current state of the pool. A 404 from yog-api
 * passes through unchanged and means "no swap or liquidity event has
 * been observed for this pool yet" — note that a pool can exist via
 * Claim* events without appearing in this projection. See the
 * file-level note in `src/lib/api/latest-state.ts`.
 */

import { NextResponse } from "next/server";

import { ApiClientError } from "@/lib/api/errors";
import { mapApiClientErrorToHttp } from "@/lib/api/http-mapping";
import { fetchPoolLatestState } from "@/lib/api/latest-state";
import { isValidPoolAddress } from "@/lib/api/pool";

export const dynamic = "force-dynamic";

type RouteParams = { params: Promise<{ address: string }> };

export async function GET(
  _request: Request,
  { params }: RouteParams,
): Promise<NextResponse> {
  const { address } = await params;

  if (!isValidPoolAddress(address)) {
    return NextResponse.json(
      { error: `invalid pool address: ${address}`, kind: "bad_request" as const },
      { status: 400 },
    );
  }

  try {
    const state = await fetchPoolLatestState(address);
    return NextResponse.json(state);
  } catch (err) {
    if (err instanceof ApiClientError) {
      console.error(
        `[BFF] /api/pools/${address}/latest-state failed:`,
        err.message,
        err.details,
      );
      const { status, body } = mapApiClientErrorToHttp(err);
      return NextResponse.json(body, { status });
    }

    console.error(
      `[BFF] /api/pools/${address}/latest-state unexpected error:`,
      err,
    );
    return NextResponse.json(
      { error: "internal server error", kind: "bad_gateway" as const },
      { status: 500 },
    );
  }
}