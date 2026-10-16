/**
 * The URL-agnostic core of the yog-api client — timeout, error classification, RFC 9457
 * parsing, zod validation — shared by the server and browser clients, which build the URL.
 */

import * as z from "zod";

import { ApiClientError } from "../errors";
import { zProblemDetailsResponse } from "../schema/zod.gen";

/** The `detail` of a non-2xx Problem Details body, or null when the body does not parse. */
async function readRemoteErrorMessage(response: Response): Promise<string | null> {
  try {
    const body: unknown = await response.json();
    const parsed = zProblemDetailsResponse.safeParse(body);
    return parsed.success ? parsed.data.detail : null;
  } catch {
    return null;
  }
}

function formatZodIssues(error: z.ZodError): string[] {
  return error.issues.map((issue) => {
    const path = issue.path.length === 0 ? "(root)" : issue.path.join(".");
    return `${path}: ${issue.message}`;
  });
}

/**
 * GETs an absolute URL and validates the body against `schema`.
 * @throws ApiClientError on a timeout, a network failure, a non-2xx or a validation failure.
 */
export async function apiGetWithUrl<T>(
  url: string,
  schema: z.ZodType<T>,
  timeoutMs: number,
): Promise<T> {

  let response: Response;
  try {
    response = await fetch(url, {
      method: "GET",
      headers: { Accept: "application/json" },
      signal: AbortSignal.timeout(timeoutMs),
      cache: "no-store",
    });
  } catch (err) {
    // `AbortSignal.timeout` rejects with a DOMException named "TimeoutError".
    if (err instanceof DOMException && err.name === "TimeoutError") {
      throw ApiClientError.timeout(timeoutMs);
    }
    throw ApiClientError.network(err);
  }

  if (!response.ok) {
    const remoteMessage = await readRemoteErrorMessage(response);
    throw ApiClientError.http(response.status, remoteMessage);
  }

  // Invalid JSON is a validation failure.
  let body: unknown;
  try {
    body = await response.json();
  } catch (err) {
    throw ApiClientError.validation([
      `response body is not valid JSON: ${err instanceof Error ? err.message : String(err)}`,
    ]);
  }

  const parsed = schema.safeParse(body);
  if (!parsed.success) {
    throw ApiClientError.validation(formatZodIssues(parsed.error));
  }

  return parsed.data;
}

/** Query-string scalars; the caller serialises anything else. */
export type QueryValue = string | number | boolean;

/**
 * Joins a base URL without a trailing slash (the env schema checks it), a path starting with
 * one, and a query bag whose undefined values are dropped.
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

/**
 * Throws a `RangeError` unless `value` passes `schema`, the generated zod of a
 * bounded query parameter, whose own bounds the message quotes.
 */
export function assertInBounds(name: string, value: number, schema: z.ZodNumber): void {
  if (!schema.safeParse(value).success) {
    throw new RangeError(
      `\`${name}\` must be an integer in [${schema.minValue}, ${schema.maxValue}], got ${value}`,
    );
  }
}
