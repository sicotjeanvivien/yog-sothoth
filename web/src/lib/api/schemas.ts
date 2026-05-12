/**
 * Zod schemas mirroring the wire shapes returned by `yog-api`.
 *
 * These schemas are the boundary contract between Next.js (BFF) and
 * the Rust HTTP service. They serve three purposes:
 *
 *   1. Runtime validation — every response from `yog-api` is parsed
 *      before being consumed, so a schema drift in the Rust handlers
 *      surfaces as a clear `ApiClientError("validation")` rather than
 *      a silent `undefined` deep in the rendering tree.
 *   2. TypeScript types — the consumer types are derived from the
 *      schemas via `z.infer`, keeping a single source of truth.
 *   3. Documentation — the file lists every DTO with comments tying
 *      it back to the Rust struct it mirrors.
 *
 * When `yog-api` changes a DTO, this file must be updated in lockstep.
 * The `.sqlx/`-style verified cache equivalent here is `pools.test.ts`
 * which exercises a representative payload end-to-end.
 */

import * as z from "zod";

// ─────────────────────────────────────────────────────────────────────
// PoolResponse — mirrors `api::http::dto::response::pool_response::PoolResponse`
// ─────────────────────────────────────────────────────────────────────

/**
 * Wire shape of a pool as exposed by `GET /api/pools`.
 *
 * Rust side (api/src/http/dto/response/pool_response.rs):
 *
 * ```rust
 * pub struct PoolResponse {
 *     address: String,
 *     protocol: String,
 *     token_a_mint: String,
 *     token_b_mint: String,
 *     first_seen_at: DateTime<Utc>,
 *     last_seen_at: DateTime<Utc>,
 * }
 * ```
 *
 * `address`, `protocol`, and the two mints are non-empty strings.
 * Timestamps are RFC3339 (chrono's default `Serialize` impl).
 */
export const PoolResponseSchema = z.object({
  address: z.string().min(1),
  protocol: z.string().min(1),
  token_a_mint: z.string().min(1),
  token_b_mint: z.string().min(1),
  // `z.iso.datetime()` enforces RFC3339 with timezone info. The Rust
  // `chrono::DateTime<Utc>` serializer always produces this shape.
  first_seen_at: z.iso.datetime({ offset: true }),
  last_seen_at: z.iso.datetime({ offset: true }),
});

export type PoolResponse = z.infer<typeof PoolResponseSchema>;

// ─────────────────────────────────────────────────────────────────────
// PageResponse<T> — mirrors `api::http::dto::response::page_response::PageResponse<T>`
// ─────────────────────────────────────────────────────────────────────

/**
 * Generic paginated envelope. `next_cursor` is `null` when the current
 * page is the last one, an opaque base64 string otherwise.
 *
 * Defined as a factory because zod 4 schemas are not generic in the
 * TypeScript sense; we compose a fresh schema per item type instead.
 */
export function pageResponseSchema<T extends z.ZodTypeAny>(item: T) {
  return z.object({
    items: z.array(item),
    next_cursor: z.string().nullable(),
  });
}

/** Paginated response of pools, used by `GET /api/pools`. */
export const PoolsPageSchema = pageResponseSchema(PoolResponseSchema);

export type PoolsPage = z.infer<typeof PoolsPageSchema>;

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