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

export type LiquidityEventResponse = z.infer<typeof LiquidityEventSchema>;
