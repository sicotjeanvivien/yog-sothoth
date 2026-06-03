/**
 * URL-agnostic core of the yog-api HTTP client.
 *
 * The two runtime-specific clients (`client.ts` for server-side,
 * `browser/client.ts` for browser-side) reach yog-api through
 * different URLs but share everything else: timeout handling,
 * error classification, RFC 9457 error envelope parsing, zod
 * validation of successful responses.
 *
 * This module exposes `apiGetWithUrl`, which takes an already-built
 * absolute URL and a timeout, and runs the common pipeline. Each
 * runtime-specific client is responsible for building the URL (and
 * for picking its own env var as the base) before calling in.
 */

import * as z from "zod";

import { ApiClientError } from "../errors";
import { ApiErrorBodySchema } from "../schema/api-error-body";

/**
 * Try to read the RFC 9457 Problem Details body returned by yog-api
 * on non-2xx responses. Returns the `detail` field — the per-occurrence
 * human-readable message. Returns null if the body is missing, malformed,
 * or does not match the expected shape — the caller still has the HTTP
 * status code to report.
 */
async function readRemoteErrorMessage(response: Response): Promise<string | null> {
  try {
    const body: unknown = await response.json();
    const parsed = ApiErrorBodySchema.safeParse(body);
    return parsed.success ? parsed.data.detail : null;
  } catch {
    return null;
  }
}

/**
 * Format zod issues into a flat list of human-readable strings,
 * compatible with `ApiClientError.validation`.
 */
function formatZodIssues(error: z.ZodError): string[] {
  return error.issues.map((issue) => {
    const path = issue.path.length === 0 ? "(root)" : issue.path.join(".");
    return `${path}: ${issue.message}`;
  });
}

/**
 * Perform a GET request against the given absolute URL and validate
 * the response body against the given schema.
 *
 * Runtime-agnostic: the only inputs are a fully-qualified URL and a
 * timeout in milliseconds. Both server-side and browser-side clients
 * call into this function after composing their URL from their own
 * env var.
 *
 * @throws ApiClientError on any failure (timeout, network, non-2xx, validation).
 */
export async function apiGetWithUrl<T>(
  url: string,
  schema: z.ZodType<T>,
  timeoutMs: number,
): Promise<T> {

  // ── Send the request, classifying transport failures ───────────────
  let response: Response;
  try {
    response = await fetch(url, {
      method: "GET",
      headers: { Accept: "application/json" },
      signal: AbortSignal.timeout(timeoutMs),
      // No browser/Next.js cache: each call sees the latest data.
      // Higher-level cache strategies (route-segment config, custom
      // wrappers) can layer on top if needed.
      cache: "no-store",
    });
  } catch (err) {
    // `AbortSignal.timeout` rejects with a `TimeoutError` (a DOMException
    // with name "TimeoutError"). Anything else is a transport failure.
    if (err instanceof DOMException && err.name === "TimeoutError") {
      throw ApiClientError.timeout(timeoutMs);
    }
    throw ApiClientError.network(err);
  }

  // ── Handle non-2xx — read the remote envelope when we can ──────────
  if (!response.ok) {
    const remoteMessage = await readRemoteErrorMessage(response);
    throw ApiClientError.http(response.status, remoteMessage);
  }

  // ── Parse the body, treating invalid JSON as a validation failure ──
  let body: unknown;
  try {
    body = await response.json();
  } catch (err) {
    throw ApiClientError.validation([
      `response body is not valid JSON: ${err instanceof Error ? err.message : String(err)}`,
    ]);
  }

  // ── Validate against the schema ────────────────────────────────────
  const parsed = schema.safeParse(body);
  console.log(parsed.error);
  
  if (!parsed.success) {
    throw ApiClientError.validation(formatZodIssues(parsed.error));
  }

  return parsed.data;
}

/**
 * Query-string-compatible scalar values. Other types (objects, arrays)
 * must be serialised by the caller — we keep the surface narrow on
 * purpose to avoid implicit conventions.
 */
export type QueryValue = string | number | boolean;

/**
 * Build a fully qualified URL by joining a base URL, a path and a
 * query bag. Both runtime-specific clients use this to avoid
 * duplicating URL composition logic.
 *
 * The base URL is expected to have no trailing slash (validated by
 * the corresponding env schema); the path is expected to start with
 * a slash. Undefined query values are dropped so callers can pass
 * optional fields without a conditional spread.
 */
export function buildUrl(
  baseUrl: string,
  path: string,
  query: Record<string, QueryValue | undefined>,
): string {
  const url = new URL(`${baseUrl}${path}`);
  for (const [key, value] of Object.entries(query)) {
    if (value !== undefined) {
      url.searchParams.set(key, String(value));
    }
  }
  return url.toString();
}