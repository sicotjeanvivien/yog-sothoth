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
  // All u64 token quantities (amounts, reserves, fees) are emitted as
  // digit-only strings: an atomic-unit value can exceed 2^53, which a
  // JSON-number consumer truncates. Same handling as `nextSqrtPrice`.
  // Use `BigInt(value)` for arithmetic.
  amountA: U128String,
  amountB: U128String,

  reserveAAfter: U128String,
  reserveBAfter: U128String,
  nextSqrtPrice: U128String,

  claimingFee: U128String,
  protocolFee: U128String,
  compoundingFee: U128String,
  referralFee: U128String,
  feeTokenIsA: z.boolean(),
});

export type SwapEventResponse = z.infer<typeof SwapEventSchema>;