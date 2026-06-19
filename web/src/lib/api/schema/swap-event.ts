import * as z from "zod";
import { Rfc3339, U128String } from "./shared";

// ─────────────────────────────────────────────────────────────────────
// SwapEventResponse — mirrors `api::http::dto::response::SwapEventResponse`
// ─────────────────────────────────────────────────────────────────────

/**
 * Wire shape of a single swap event in the per-pool feed.
 */
export const SwapEventSchema = z.object({
  poolAddress: z.string().min(1),
  protocol: z.string().min(1),
  signature: z.string().min(1),
  timestamp: Rfc3339,

  tradeDirection: z.enum(["a_to_b", "b_to_a"]),
  amountA: z.number().int().nonnegative(),
  amountB: z.number().int().nonnegative(),

  // Reserves are `u64` emitted as digit-only strings: a pool balance in
  // atomic units can exceed 2^53, which a JSON-number consumer truncates.
  // Same handling as `nextSqrtPrice`. Use `BigInt(value)` for arithmetic.
  reserveAAfter: U128String,
  reserveBAfter: U128String,
  nextSqrtPrice: U128String,

  claimingFee: z.number().int().nonnegative(),
  protocolFee: z.number().int().nonnegative(),
  compoundingFee: z.number().int().nonnegative(),
  referralFee: z.number().int().nonnegative(),
  feeTokenIsA: z.boolean(),
});

export type SwapEventResponse = z.infer<typeof SwapEventSchema>;