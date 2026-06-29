import * as z from "zod";
import { BigDecimal, Rfc3339, U128String } from "./shared";

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
  // u64 quantities emitted as digit-only strings (can exceed 2^53); see
  // SwapEventSchema. Use `BigInt(value)` for arithmetic.
  amountA: U128String,
  amountB: U128String,
  liquidityDelta: U128String,

  reserveAAfter: U128String,
  reserveBAfter: U128String,

  position: z.string().min(1),
  owner: z.string().min(1),

  // Trade-time USD value of the event (both legs at the price as-of the
  // event), derived server-side. `null` when a leg is unpriced or the
  // mints/decimals are unresolved → the table renders "—". Decimal string.
  valueUsd: BigDecimal.nullable(),
});

export type LiquidityEventResponse = z.infer<typeof LiquidityEventSchema>;
