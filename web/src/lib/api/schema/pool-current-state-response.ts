import * as z from "zod";
import { Rfc3339, U128String } from "./shared";

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