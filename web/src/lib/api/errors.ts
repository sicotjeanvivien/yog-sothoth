/**
 * The typed failure of a yog-api call, internal to the Next.js runtime: a timeout or a network
 * failure has no HTTP status, so it is not modelled on RFC 9457.
 */

/** `http` is a non-2xx answer; `validation` a 2xx whose body fails the schema. */
export type ApiClientErrorKind = "timeout" | "network" | "http" | "validation";

export type ApiClientErrorDetails =
  | { kind: "timeout"; timeoutMs: number }
  | { kind: "network"; cause: unknown }
  | { kind: "http"; status: number; remoteMessage: string | null }
  | { kind: "validation"; issues: string[] };

/** The message is for server logs. */
export class ApiClientError extends Error {
  public readonly details: ApiClientErrorDetails;

  constructor(message: string, details: ApiClientErrorDetails) {
    super(message);
    this.name = "ApiClientError";
    this.details = details;
  }

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