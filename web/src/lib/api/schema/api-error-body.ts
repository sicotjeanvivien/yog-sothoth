import * as z from "zod";

// ─────────────────────────────────────────────────────────────────────
// ProblemDetailsBody — RFC 9457 envelope returned by yog-api
// ─────────────────────────────────────────────────────────────────────

/**
 * Error envelope sent by `yog-api` on non-2xx responses.
 *
 * Conforms to [RFC 9457 Problem Details for HTTP APIs](https://www.rfc-editor.org/rfc/rfc9457).
 *
 * Content-Type on the wire: `application/problem+json`.
 *
 * Rust side (api/src/http/dto/response/problem.rs):
 *
 * ```rust
 * pub(crate) struct ProblemDetails {
 *     pub(crate) type_uri: &'static str,   // serialized as "type"
 *     pub(crate) title: &'static str,
 *     pub(crate) status: u16,
 *     pub(crate) detail: String,
 * }
 * ```
 *
 * Parsing is best-effort: a malformed error body must not mask the
 * underlying HTTP status code. `readRemoteErrorMessage` falls back to
 * `null` when this schema does not match, leaving the status code as
 * the sole signal.
 *
 * At this stage, `type` is always `"about:blank"` and discrimination
 * is carried by `title`. Future yog-api versions will introduce
 * specific type URIs (e.g. `https://api.yog-sothoth.fr/errors/...`);
 * the schema below already permits any string in `type` so no
 * breaking change is required when that happens.
 */
export const ApiErrorBodySchema = z.object({
  type: z.string(),
  title: z.string(),
  status: z.number().int().nonnegative(),
  detail: z.string(),
});

export type ApiErrorBodyResponse = z.infer<typeof ApiErrorBodySchema>;