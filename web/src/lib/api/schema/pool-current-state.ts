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
 * `reserveA`/`reserveB` (u64) and `lastSqrtPrice`/`liquidity` (u128) are
 * all emitted as digit-only strings to survive the JS 2^53 ceiling.
 */
export const PoolCurrentStateSchema = z.object({
  poolAddress: z.string().min(1),
  protocol: z.string().min(1),

  lastEventAt: Rfc3339,
  // Closed set on the Rust side (`LastEventKind::as_str`); reject drift.
  lastEventKind: z.enum(["swap", "liquidity_add", "liquidity_remove"]),
  lastSignature: z.string().min(1),

  // u64 reserves emitted as digit-only strings (can exceed 2^53); see
  // the file header. Use `BigInt(value)` for arithmetic.
  reserveA: U128String,
  reserveB: U128String,

  lastSqrtPrice: U128String.nullable(),
  lastSwapAt: Rfc3339.nullable(),

  liquidity: U128String.nullable(),
  lastLiquidityAt: Rfc3339.nullable(),

  updatedAt: Rfc3339,
});

export type PoolCurrentStateResponse = z.infer<typeof PoolCurrentStateSchema>;