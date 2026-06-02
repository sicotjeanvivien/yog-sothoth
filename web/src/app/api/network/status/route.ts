/**
 * BFF route handler for `GET /api/network/status`.
 *
 * Sits between the browser and `yog-api`. Responsibilities:
 *
 *   1. Delegate to `fetchNetworkStatus` for the upstream call.
 *   2. Return the typed snapshot on success, or a translated HTTP
 *      error via `mapApiClientErrorToHttp` on failure.
 *
 * There is no input to parse — the endpoint takes no query string —
 * so this handler is even thinner than `/api/pools`: no `parseQuery`
 * step. Every reusable piece (URL building, timeout, schema
 * validation, error classification) still lives in `src/lib/api/`.
 *
 * Lives under `app/api/network/status/route.ts` (not
 * `app/[locale]/api/...`): API resources are locale-agnostic — the
 * browser always fetches the same path regardless of active locale.
 * Error message localisation happens in React via next-intl, keyed
 * on the typed `kind` field returned in the error body.
 */

import { NextResponse } from "next/server";
import { ApiClientError } from "@/lib/api/errors";
import { internalErrorProblem, mapApiClientErrorToHttp, problemResponse } from "@/lib/api/http-mapping";
import { fetchNetworkStatus } from "@/lib/api/network-status";

export async function GET(): Promise<NextResponse> {
  try {
    const status = await fetchNetworkStatus();
    return NextResponse.json(status);
  } catch (err) {
    if (err instanceof ApiClientError) {
      // Full internal detail server-side; the browser gets the
      // sanitised version from `mapApiClientErrorToHttp`.
      console.error(
        "[BFF] /api/network/status failed:",
        err.message,
        err.details,
      );
      const { status, body } = mapApiClientErrorToHttp(err);
      return problemResponse(body, { status });
    }

    // Anything not an ApiClientError is an unexpected programmer
    // error in the BFF itself, not a 4xx for the browser.
    console.error("[BFF] /api/network/status unexpected error:", err);
    return problemResponse(internalErrorProblem());
  }
}