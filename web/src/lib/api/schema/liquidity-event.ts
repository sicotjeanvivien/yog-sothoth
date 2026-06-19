import * as z from "zod";
import { Rfc3339, U128String } from "./shared";

// ─────────────────────────────────────────────────────────────────────
// LiquidityEventResponse — mirrors `api::http::dto::response::LiquidityEventResponse`
// ─────────────────────────────────────────────────────────────────────

/**
 * Wire shape of a single liquidity event (add or remove) in the
 * per-pool feed.
 */
export const LiquidityEventSchema = z.object({
  poolAddress: z.string().min(1),
  protocol: z.string().min(1),
  signature: z.string().min(1),
  timestamp: Rfc3339,

  liquidityEventKind: z.enum(["add", "remove"]),
  amountA: z.number().int().nonnegative(),
  amountB: z.number().int().nonnegative(),
  liquidityDelta: U128String,

  // u64 reserves emitted as digit-only strings (can exceed 2^53); see
  // SwapEventSchema. Use `BigInt(value)` for arithmetic.
  reserveAAfter: U128String,
  reserveBAfter: U128String,

  position: z.string().min(1),
  owner: z.string().min(1),
});

export type LiquidityEventResponse = z.infer<typeof LiquidityEventSchema>;
