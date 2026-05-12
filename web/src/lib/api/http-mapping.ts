/**
 * Map `ApiClientError` variants to HTTP status codes for BFF route
 * handlers.
 *
 * The BFF sits between the browser and `yog-api`. Its job, on failure,
 * is to translate the kind of failure into an HTTP shape the browser
 * can act on, without leaking internal details. The mapping below is
 * shared across every route handler so the contract is uniform.
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
 */

import type { ApiClientError } from "./errors";

/**
 * Shape returned to the browser on failure. Single envelope used by
 * every route handler, mirroring the one yog-api itself uses.
 *
 * `kind` lets the browser branch on the failure type without parsing
 * the message string — the React layer will translate `kind` into a
 * localised user-facing message via next-intl.
 */
export type BffErrorBody = {
  error: string;
  kind: "bad_request" | "not_found" | "bad_gateway" | "gateway_timeout";
};

/**
 * Resolve an `ApiClientError` into the HTTP status and body the BFF
 * route handler should return to the browser.
 *
 * Internal messages (containing remote URLs, zod issues, exception
 * stacks) are NEVER forwarded as-is. They are logged by the caller;
 * the body returned here is a stable, generic string keyed by `kind`.
 */
export function mapApiClientErrorToHttp(err: ApiClientError): {
  status: number;
  body: BffErrorBody;
} {
  switch (err.details.kind) {
    case "timeout":
      return {
        status: 504,
        body: { error: "upstream API timed out", kind: "gateway_timeout" },
      };

    case "network":
      return {
        status: 502,
        body: { error: "upstream API unreachable", kind: "bad_gateway" },
      };

    case "validation":
      // yog-api returned 200 but the payload is malformed. From the
      // browser's perspective this is indistinguishable from a 5xx —
      // the BFF cannot fulfil the request because the backend
      // contract is broken.
      return {
        status: 502,
        body: { error: "upstream API returned an unexpected response", kind: "bad_gateway" },
      };

    case "http": {
      const remoteStatus = err.details.status;

      // 4xx from yog-api → passthrough. The browser sent something
      // yog-api rejected (e.g. invalid cursor); the BFF didn't
      // synthesize the error and shouldn't hide it.
      if (remoteStatus >= 400 && remoteStatus < 500) {
        const kind = remoteStatus === 404 ? "not_found" : "bad_request";
        return {
          status: remoteStatus,
          body: {
            // Forwarding the remote message is safe here: yog-api's
            // BadRequest variants only carry validation details the
            // caller needs to see (bad cursor, bad limit, ...).
            error: err.details.remoteMessage ?? "request rejected by upstream API",
            kind,
          },
        };
      }

      // 5xx from yog-api → collapse into 502 Bad Gateway. The
      // remote message is dropped: internal errors on yog-api are
      // logged there with full context (see `error/error.rs` →
      // `error!(error = %msg, "internal API error")`).
      return {
        status: 502,
        body: { error: "upstream API error", kind: "bad_gateway" },
      };
    }
  }
}