/**
 * Typed error surface for BFF → yog-api calls.
 *
 * `ApiClientError` is an INTERNAL TypeScript class. It never crosses
 * a network boundary — it lives in the Next.js runtime, instantiated
 * by `apiGet` when an upstream call fails, caught by Server Components
 * or BFF route handlers downstream.
 *
 * The browser-facing wire format is RFC 9457 Problem Details
 * (`{ type, title, status, detail }`), produced by `mapApiClientErrorToHttp`
 * The two layers aredeliberately distinct: this class can carry information that has
 * no HTTP counterpart (a timeout has no response body, a network
 * failure has no status code), and aligning it on RFC 9457 would
 * force inventing artificial titles for things that aren't HTTP
 * problems in the first place.
 *
 * | ApiClientError kind | mapping to HTTP             |
 * |---------------------|-----------------------------|
 * | timeout             | 504 Gateway Timeout         |
 * | network             | 502 Bad Gateway             |
 * | http (4xx)          | passthrough (400, 404, ...) |
 * | http (5xx)          | 502 Bad Gateway             |
 * | validation          | 502 Bad Gateway             |
 *
 */

/**
 * Kind of failure that occurred when calling `yog-api`.
 *
 * - `timeout`: the request did not complete within the configured timeout.
 * - `network`: the request could not be sent (DNS failure, connection refused, etc.).
 * - `http`: the request completed but `yog-api` returned a non-2xx status.
 * - `validation`: the response was 2xx but the body did not match the expected schema.
 */
export type ApiClientErrorKind = "timeout" | "network" | "http" | "validation";

/**
 * Discriminated payload attached to the error, carrying the bits the
 * caller needs to react to the failure.
 */
export type ApiClientErrorDetails =
  | { kind: "timeout"; timeoutMs: number }
  | { kind: "network"; cause: unknown }
  | { kind: "http"; status: number; remoteMessage: string | null }
  | { kind: "validation"; issues: string[] };

/**
 * Error thrown by every function in `lib/api/`. The `details` field
 * carries the discriminated information; the message is a
 * human-readable summary suitable for server logs.
 */
export class ApiClientError extends Error {
  public readonly details: ApiClientErrorDetails;

  constructor(message: string, details: ApiClientErrorDetails) {
    super(message);
    this.name = "ApiClientError";
    this.details = details;
  }

  // ── Factory helpers ────────────────────────────────────────────────
  //
  // Constructors per kind, kept as static methods so call sites read
  // declaratively: `ApiClientError.timeout(5000)` instead of an inline
  // object literal.

  static timeout(timeoutMs: number): ApiClientError {
    return new ApiClientError(`yog-api request timed out after ${timeoutMs}ms`, {
      kind: "timeout",
      timeoutMs,
    });
  }

  static network(cause: unknown): ApiClientError {
    const reason = cause instanceof Error ? cause.message : String(cause);
    return new ApiClientError(`yog-api request failed to reach the server: ${reason}`, {
      kind: "network",
      cause,
    });
  }

  static http(status: number, remoteMessage: string | null): ApiClientError {
    const suffix = remoteMessage ? ` — ${remoteMessage}` : "";
    return new ApiClientError(`yog-api returned HTTP ${status}${suffix}`, {
      kind: "http",
      status,
      remoteMessage,
    });
  }

  static validation(issues: string[]): ApiClientError {
    return new ApiClientError(
      `yog-api response did not match the expected schema:\n${issues.map((i) => `  - ${i}`).join("\n")}`,
      { kind: "validation", issues },
    );
  }
}