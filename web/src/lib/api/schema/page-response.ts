import * as z from "zod";
import { PoolResponseSchema } from "./pool-response";
import { SwapEventResponseSchema } from "./swap-event-response";
import { LiquidityEventResponseSchema } from "./liquidity-event-response";

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