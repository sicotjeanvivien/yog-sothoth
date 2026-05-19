/**
 * Low-level fetch wrapper for `yog-api`.
 *
 * Every higher-level call (`fetchPools`, future `fetchSwaps`, etc.)
 * routes through `apiGet`. Responsibilities centralised here:
 *
 *   - URL composition from the validated `YOG_API_BASE_URL`
 *   - Request timeout via `AbortSignal.timeout`
 *   - Classification of failures into `ApiClientError` variants
 *   - Best-effort parsing of the remote error envelope
 *   - Schema validation of successful responses
 *
 * Higher-level functions describe *which* endpoint and *which* schema;
 * everything else lives here.
 */

import * as z from "zod";

import { ApiClientError } from "./errors";
import { ApiErrorBodySchema } from "./schema/api-error-body";
import { loadServerEnv as getServerEnv } from "../config/server-env.schema";

/**
 * Query-string-compatible scalar values. Other types (objects, arrays)
 * must be serialised by the caller — we keep the surface narrow on
 * purpose to avoid implicit conventions.
 */
type QueryValue = string | number | boolean;

/**
 * Build a fully qualified URL against `YOG_API_BASE_URL`, appending
 * the given query parameters. Undefined values are dropped so callers
 * can pass optional fields without a conditional spread.
 */
function buildUrl(path: string, query: Record<string, QueryValue | undefined>): string {
  const { YOG_API_BASE_URL } = getServerEnv();

  // The schema validates that the base URL has no trailing slash, so
  // a single forward concat is safe and readable.
  const url = new URL(`${YOG_API_BASE_URL}${path}`);

  for (const [key, value] of Object.entries(query)) {
    if (value !== undefined) {
      url.searchParams.set(key, String(value));
    }
  }

  return url.toString();
}

/**
 * Try to read the `{ "error": "..." }` body returned by yog-api on
 * non-2xx responses. Returns null if the body is missing, malformed,
 * or does not match the expected shape — the caller still has the
 * HTTP status code to report.
 */
async function readRemoteErrorMessage(response: Response): Promise<string | null> {
  try {
    const body: unknown = await response.json();
    const parsed = ApiErrorBodySchema.safeParse(body);
    return parsed.success ? parsed.data.error : null;
  } catch {
    // Body was not JSON, or the stream was already consumed. Either
    // way, we can't surface a remote message — return null and let
    // the HTTP status code carry the meaning.
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
 * Perform a GET request against `yog-api` and validate the response
 * body against the given schema.
 *
 * @throws ApiClientError on any failure (timeout, network, non-2xx, validation).
 */
export async function apiGet<T>(
  path: string,
  query: Record<string, QueryValue | undefined>,
  schema: z.ZodType<T>,
): Promise<T> {
  const { YOG_API_TIMEOUT_MS } = getServerEnv();
  const url = buildUrl(path, query);

  // ── Send the request, classifying transport failures ───────────────
  let response: Response;
  try {
    response = await fetch(url, {
      method: "GET",
      headers: { Accept: "application/json" },
      signal: AbortSignal.timeout(YOG_API_TIMEOUT_MS),
      // The default Next.js fetch cache (`force-cache`) is unwanted
      // here: we want every BFF call to see a fresh response from
      // yog-api. Individual callers can wrap us in `unstable_cache` or
      // route segment config if they want caching.
      cache: "no-store",
    });
  } catch (err) {
    // `AbortSignal.timeout` rejects with a `TimeoutError` (a DOMException
    // with name "TimeoutError"). Anything else is a transport failure.
    if (err instanceof DOMException && err.name === "TimeoutError") {
      throw ApiClientError.timeout(YOG_API_TIMEOUT_MS);
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
  if (!parsed.success) {
    throw ApiClientError.validation(formatZodIssues(parsed.error));
  }

  return parsed.data;
}