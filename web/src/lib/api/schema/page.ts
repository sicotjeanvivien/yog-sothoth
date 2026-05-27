import * as z from "zod";
import { PoolSchema } from "./pool";
import { SwapEventSchema } from "./swap-event";
import { LiquidityEventSchema } from "./liquidity-event";

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
export function pageSchema<T extends z.ZodTypeAny>(item: T) {
  return z.object({
    items: z.array(item),
    nextCursor: z.string().nullable(),
  });
}

// ── Concrete pages ────────────────────────────────────────────────────

export const PoolsPageSchema = pageSchema(PoolSchema);
export type PoolsPageResponse = z.infer<typeof PoolsPageSchema>;

export const SwapEventsPageSchema = pageSchema(SwapEventSchema);
export type SwapEventsPageResponse = z.infer<typeof SwapEventsPageSchema>;

export const LiquidityEventsPageSchema = pageSchema(LiquidityEventSchema);
export type LiquidityEventsPageResponse = z.infer<typeof LiquidityEventsPageSchema>;