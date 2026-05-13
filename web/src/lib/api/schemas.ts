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
 *
 * # u128 fields
 *
 * Rust `u128` values (sqrt_price, liquidity, liquidity_delta) are emitted
 * as JSON strings by `yog-api` to preserve precision across the JS bridge.
 * On this side, we keep them as `string` validated by a digits-only regex.
 * Components that need numeric semantics call `BigInt(value)` at the
 * point of use; components that only display the value can format the
 * string directly.
 *
 * Keeping them as strings (rather than `z.string().transform(BigInt)`)
 * avoids a serialisation footgun: BFF route handlers re-emit the parsed
 * page via `NextResponse.json(...)`, and `JSON.stringify` throws on
 * bigint values. Deferring the conversion to the call site solves both
 * problems cleanly.
 */

import * as z from "zod";

// ─────────────────────────────────────────────────────────────────────
// Shared primitives
// ─────────────────────────────────────────────────────────────────────

/**
 * A decimal u128 as emitted by yog-api: a non-empty digit-only string.
 * Components that need numeric semantics call `BigInt(value)` themselves.
 */
const U128String = z.string().regex(/^\d+$/, "expected a non-negative decimal integer");

/** RFC 3339 timestamp with timezone offset — matches Rust's `chrono::DateTime<Utc>` output. */
const Rfc3339 = z.iso.datetime({ offset: true });

// ─────────────────────────────────────────────────────────────────────
// PoolResponse — mirrors `api::http::dto::response::PoolResponse`
// ─────────────────────────────────────────────────────────────────────

/**
 * Wire shape of a pool as exposed by `GET /api/pools` and
 * `GET /api/pools/{address}`.
 *
 * Rust side (api/src/http/dto/response.rs):
 *
 * ```rust
 * pub struct PoolResponse {
 *     pool_address: String,
 *     protocol: String,
 *     token_a_mint: String,
 *     token_b_mint: String,
 *     first_seen_at: DateTime<Utc>,
 *     last_seen_at: DateTime<Utc>,
 * }
 * ```
 */
export const PoolResponseSchema = z.object({
  pool_address: z.string().min(1),
  protocol: z.string().min(1),
  token_a_mint: z.string().min(1),
  token_b_mint: z.string().min(1),
  first_seen_at: Rfc3339,
  last_seen_at: Rfc3339,
});

export type PoolResponse = z.infer<typeof PoolResponseSchema>;

// ─────────────────────────────────────────────────────────────────────
// PoolCurrentStateResponse — mirrors `api::http::dto::response::PoolCurrentStateResponse`
// ─────────────────────────────────────────────────────────────────────

/**
 * Wire shape of a pool's latest projected state, as exposed by
 * `GET /api/pools/{address}/latest-state`.
 *
 * Returns 404 if no swap or liquidity event has been observed for the
 * pool yet — note that a pool may exist via Claim* events without
 * appearing in this projection (see CQRS read model in
 * `crates/core/src/domain/pool_current_state.rs`).
 *
 * `last_sqrt_price` and `liquidity` are emitted as digit-only strings;
 * see the file-level note on u128 handling.
 */
export const PoolCurrentStateResponseSchema = z.object({
  pool_address: z.string().min(1),
  protocol: z.string().min(1),

  last_event_at: Rfc3339,
  last_event_kind: z.enum(["swap", "liquidity_add", "liquidity_remove"]),
  last_signature: z.string().min(1),

  reserve_a: z.number().int().nonnegative(),
  reserve_b: z.number().int().nonnegative(),

  last_sqrt_price: U128String.nullable(),
  last_swap_at: Rfc3339.nullable(),

  liquidity: U128String.nullable(),
  last_liquidity_at: Rfc3339.nullable(),

  updated_at: Rfc3339,
});

export type PoolCurrentStateResponse = z.infer<typeof PoolCurrentStateResponseSchema>;

// ─────────────────────────────────────────────────────────────────────
// SwapEventResponse — mirrors `api::http::dto::response::SwapEventResponse`
// ─────────────────────────────────────────────────────────────────────

/**
 * Wire shape of a single swap event in the per-pool feed.
 */
export const SwapEventResponseSchema = z.object({
  pool_address: z.string().min(1),
  protocol: z.string().min(1),
  signature: z.string().min(1),
  timestamp: Rfc3339,

  token_a_mint: z.string().min(1),
  token_b_mint: z.string().min(1),

  trade_direction: z.enum(["a_to_b", "b_to_a"]),
  amount_a: z.number().int().nonnegative(),
  amount_b: z.number().int().nonnegative(),

  reserve_a_after: z.number().int().nonnegative(),
  reserve_b_after: z.number().int().nonnegative(),
  next_sqrt_price: U128String,

  claiming_fee: z.number().int().nonnegative(),
  protocol_fee: z.number().int().nonnegative(),
  compounding_fee: z.number().int().nonnegative(),
  referral_fee: z.number().int().nonnegative(),
  fee_token_is_a: z.boolean(),
});

export type SwapEventResponse = z.infer<typeof SwapEventResponseSchema>;

// ─────────────────────────────────────────────────────────────────────
// LiquidityEventResponse — mirrors `api::http::dto::response::LiquidityEventResponse`
// ─────────────────────────────────────────────────────────────────────

/**
 * Wire shape of a single liquidity event (add or remove) in the
 * per-pool feed.
 */
export const LiquidityEventResponseSchema = z.object({
  pool_address: z.string().min(1),
  protocol: z.string().min(1),
  signature: z.string().min(1),
  timestamp: Rfc3339,

  token_a_mint: z.string().min(1),
  token_b_mint: z.string().min(1),

  liquidity_event_kind: z.enum(["add", "remove"]),
  amount_a: z.number().int().nonnegative(),
  amount_b: z.number().int().nonnegative(),
  liquidity_delta: U128String,

  reserve_a_after: z.number().int().nonnegative(),
  reserve_b_after: z.number().int().nonnegative(),

  position: z.string().min(1),
  owner: z.string().min(1),
});

export type LiquidityEventResponse = z.infer<typeof LiquidityEventResponseSchema>;

// ─────────────────────────────────────────────────────────────────────
// PageResponse<T> — mirrors `api::http::dto::response::PageResponse<T>`
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

// ── Concrete pages ────────────────────────────────────────────────────

export const PoolsPageSchema = pageResponseSchema(PoolResponseSchema);
export type PoolsPage = z.infer<typeof PoolsPageSchema>;

export const SwapEventsPageSchema = pageResponseSchema(SwapEventResponseSchema);
export type SwapEventsPage = z.infer<typeof SwapEventsPageSchema>;

export const LiquidityEventsPageSchema = pageResponseSchema(LiquidityEventResponseSchema);
export type LiquidityEventsPage = z.infer<typeof LiquidityEventsPageSchema>;

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