import * as z from "zod";

// ─────────────────────────────────────────────────────────────────────
// ApiErrorBody — mirrors the `{ "error": "..." }` envelope from yog-api
// ─────────────────────────────────────────────────────────────────────

/**
 * Error envelope sent by `yog-api` on non-2xx responses.
 *
 * Rust side (api/src/http/error.rs):
 *
 * ```rust
 * (status, Json(json!({ "error": message }))).into_response()
 * ```
 *
 * Parsed best-effort: a malformed error body should not mask the
 * underlying HTTP status code, so consumers fall back to a generic
 * message when this fails to parse.
 */
export const ApiErrorBodySchema = z.object({
  error: z.string(),
});

export type ApiErrorBody = z.infer<typeof ApiErrorBodySchema>;