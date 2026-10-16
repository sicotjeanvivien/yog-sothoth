/**
 * An API call as an outcome to branch on, without try/catch in a Server Component. ⚠️ Anything
 * but an `ApiClientError` is re-thrown, to reach Next.js's error boundary.
 */

import { ApiClientError, type ApiClientErrorKind } from "./errors";

export type SafeFetchOutcome<T> =
  | { kind: "ok"; data: T }
  | { kind: "error"; reason: ApiClientErrorKind };

export async function safeFetch<T>(
  fn: () => Promise<T>,
): Promise<SafeFetchOutcome<T>> {
  try {
    const data = await fn();
    return { kind: "ok", data };
  } catch (err) {
    if (err instanceof ApiClientError) {
      return { kind: "error", reason: err.details.kind };
    }
    throw err;
  }
}

/** For a lookup: an upstream 404 is `not_found`, not an error. */
export type SafeFetchOrNotFoundOutcome<T> =
  | { kind: "ok"; data: T }
  | { kind: "not_found" }
  | { kind: "error"; reason: ApiClientErrorKind };

export async function safeFetchOrNotFound<T>(
  fn: () => Promise<T>,
): Promise<SafeFetchOrNotFoundOutcome<T>> {
  try {
    const data = await fn();
    return { kind: "ok", data };
  } catch (err) {
    if (err instanceof ApiClientError) {
      if (err.details.kind === "http" && err.details.status === 404) {
        return { kind: "not_found" };
      }
      return { kind: "error", reason: err.details.kind };
    }
    throw err;
  }
}