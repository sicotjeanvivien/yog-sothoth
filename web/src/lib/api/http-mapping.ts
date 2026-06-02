/**
 * Map `ApiClientError` variants to RFC 9457 Problem Details bodies
 * for BFF route handlers.
 *
 * The BFF sits between the browser and `yog-api`. On failure, its
 * job is to translate the kind of failure into an HTTP shape the
 * browser can act on, without leaking internal details. The mapping
 * below is shared across every route handler so the contract is
 * uniform — and matches the format `yog-api` itself uses for its
 * own errors, so the dashboard speaks a single error dialect across
 * its whole API surface.
 *
 * Convention:
 *   - 4xx from yog-api passes through unchanged (the browser sent
 *     something the backend rejected — same goes for the browser
 *     facing the BFF).
 *   - 5xx from yog-api collapses into a single 502 Bad Gateway from
 *     the BFF — the browser shouldn't distinguish between "yog-api
 *     crashed" and "yog-api returned 500", they're both gateway
 *     failures from its perspective.
 *   - Transport failures (timeout, network) surface as 504/502.
 *   - Schema validation failures (yog-api drifted from its contract)
 *     surface as 502 — same client-facing meaning as a 5xx, since the
 *     BFF cannot trust what the backend returned.
 *
 * Reference: <https://www.rfc-editor.org/rfc/rfc9457>
 */

import { NextResponse } from "next/server";

import type { ApiClientError } from "./errors";

// ─────────────────────────────────────────────────────────────────────
// Wire shape — RFC 9457 Problem Details
// ─────────────────────────────────────────────────────────────────────

/**
 * RFC 9457 Problem Details content type. Set on every error response
 * returned by the BFF, distinct from `application/json` used for
 * successful payloads.
 */
export const PROBLEM_CONTENT_TYPE = "application/problem+json";

/**
 * Shape of the error body returned to the browser. Matches the
 * format yog-api uses for its own errors (see Rust
 * `http::dto::response::problem::ProblemDetails`).
 *
 * Field semantics (RFC 9457 §3):
 *   - `type` — URI reference identifying the problem type. Currently
 *     always `"about:blank"`, meaning "no specific type, see title".
 *   - `title` — short, human-readable summary. Stable across
 *     occurrences of the same problem type. This is what the browser
 *     branches on for type-based discrimination.
 *   - `status` — the HTTP status code returned by the BFF (NOT the
 *     upstream status — for a timeout that becomes a 504, status is
 *     504 here even though upstream may have returned anything).
 *   - `detail` — human-readable per-occurrence message.
 */
export type ProblemDetailsBody = {
  type: string;
  title: string;
  status: number;
  detail: string;
};

// ─────────────────────────────────────────────────────────────────────
// Response helpers
// ─────────────────────────────────────────────────────────────────────

/**
 * Build a NextResponse carrying a Problem Details body with the
 * correct content type. Use this anywhere a route handler returns
 * an error — never reach for `NextResponse.json(...)` for errors,
 * because that would emit `application/json` and break the RFC
 * contract.
 */
export function problemResponse(
  body: ProblemDetailsBody,
  init?: { status?: number },
): NextResponse {
  return new NextResponse(JSON.stringify(body), {
    status: init?.status ?? body.status,
    headers: { "content-type": PROBLEM_CONTENT_TYPE },
  });
}

/**
 * Build a Problem Details body with `type = "about:blank"`. The
 * `title` carries the discrimination role at this stage. When
 * specific problem types are introduced later, a separate helper
 * (or an optional `type` argument) will accommodate them.
 */
function problem(title: string, status: number, detail: string): ProblemDetailsBody {
  return { type: "about:blank", title, status, detail };
}

// ─────────────────────────────────────────────────────────────────────
// Local-validation helpers (no upstream call involved)
// ─────────────────────────────────────────────────────────────────────

/**
 * Build a 400 Problem Details body for a local validation failure
 * (e.g. malformed query parameter caught before the upstream call).
 * `detail` carries the client-facing message; keep it concrete and
 * actionable.
 */
export function badRequestProblem(detail: string): ProblemDetailsBody {
  return problem("Bad Request", 400, detail);
}

/**
 * Build a 500 Problem Details body for an unexpected, non-`ApiClientError`
 * failure inside the BFF route handler itself. Caller is expected to
 * have logged the underlying error before producing this.
 */
export function internalErrorProblem(): ProblemDetailsBody {
  return problem("Internal Server Error", 500, "internal server error");
}

// ─────────────────────────────────────────────────────────────────────
// ApiClientError mapping
// ─────────────────────────────────────────────────────────────────────

/**
 * Resolve an `ApiClientError` into a Problem Details body and the
 * HTTP status to return.
 *
 * Internal messages (containing remote URLs, zod issues, exception
 * stacks) are NEVER forwarded as-is. They are logged by the caller;
 * the body returned here is a stable, generic string keyed by title.
 */
export function mapApiClientErrorToHttp(err: ApiClientError): {
  status: number;
  body: ProblemDetailsBody;
} {
  switch (err.details.kind) {
    case "timeout":
      return {
        status: 504,
        body: problem("Gateway Timeout", 504, "upstream API timed out"),
      };

    case "network":
      return {
        status: 502,
        body: problem("Bad Gateway", 502, "upstream API unreachable"),
      };

    case "validation":
      // yog-api returned 2xx but the payload is malformed. From the
      // browser's perspective this is indistinguishable from a 5xx —
      // the BFF cannot fulfil the request because the backend
      // contract is broken.
      return {
        status: 502,
        body: problem(
          "Bad Gateway",
          502,
          "upstream API returned an unexpected response",
        ),
      };

    case "http": {
      const remoteStatus = err.details.status;

      // 4xx from yog-api → passthrough. The browser sent something
      // yog-api rejected (e.g. invalid cursor); the BFF didn't
      // synthesize the error and shouldn't hide it.
      if (remoteStatus >= 400 && remoteStatus < 500) {
        const title = remoteStatus === 404 ? "Not Found" : "Bad Request";
        return {
          status: remoteStatus,
          body: problem(
            title,
            remoteStatus,
            // Forwarding the remote message is safe here: yog-api's
            // BadRequest variants only carry validation details the
            // caller needs to see (bad cursor, bad limit, ...).
            err.details.remoteMessage ?? "request rejected by upstream API",
          ),
        };
      }

      // 5xx from yog-api → collapse into 502 Bad Gateway. The
      // remote message is dropped: internal errors on yog-api are
      // logged there with full context (see Rust
      // `http/error.rs` → `error!(error = %msg, "internal API error")`).
      return {
        status: 502,
        body: problem("Bad Gateway", 502, "upstream API error"),
      };
    }
  }
}