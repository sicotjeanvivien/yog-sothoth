import * as z from "zod";
import { Rfc3339, U128String } from "./shared";

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