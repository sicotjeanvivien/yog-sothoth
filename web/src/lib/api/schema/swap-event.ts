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

  // Both reserves are `u64` on the wire (JSON numbers); model the two
  // sides identically. See the u64/2^53 note in the `next_sqrt_price`
  // file header on the Rust DTO.
  reserveAAfter: z.number().int().nonnegative(),
  reserveBAfter: z.number().int().nonnegative(),
  nextSqrtPrice: U128String,

  claimingFee: z.number().int().nonnegative(),
  protocolFee: z.number().int().nonnegative(),
  compoundingFee: z.number().int().nonnegative(),
  referralFee: z.number().int().nonnegative(),
  feeTokenIsA: z.boolean(),
});

export type SwapEventResponse = z.infer<typeof SwapEventSchema>;