/**
 * Wrap an API call in a `Result`-like outcome so the caller can
 * branch on success vs typed failure without writing try/catch in
 * server components.
 *
 *   const outcome = await safeFetch(() => fetchPoolSwapEvents(address));
 *   if (outcome.kind === "ok") render(outcome.data);
 *   else                     render(<BlockError kind={outcome.reason} />);
 *
 * Anything that isn't an `ApiClientError` is re-thrown — unexpected
 * failures should surface in Next.js' error boundary, not be
 * collapsed into a generic block-level UI.
 *
 * This helper is intentionally small (one function) and lives next
 * to the rest of the API layer; if more error-shaping logic ever
 * needs to be added, this is the right place.
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

/**
 * Specialised variant for the "find this resource" case where a
 * 404 from the upstream means "doesn't exist" rather than "fetch
 * failed". Returns `null` on 404 instead of an error outcome.
 *
 *   const outcome = await safeFetchOrNotFound(() => fetchPool(addr));
 *   if (outcome.kind === "not_found") notFound();
 *   if (outcome.kind === "error")     return <PageError ... />;
 *   render(outcome.data);
 */
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