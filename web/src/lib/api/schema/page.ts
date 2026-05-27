import * as z from "zod";
import { PoolSchema } from "./pool";
import { SwapEventSchema } from "./swap-event";
import { LiquidityEventSchema } from "./liquidity-event";

// ─────────────────────────────────────────────────────────────────────
// PageResponse<T> — mirrors `api::http::dto::response::PageResponse<T>`
// ─────────────────────────────────────────────────────────────────────

/**
 * Wire shape of a paginated response.
 *
 * Bidirectional pagination: every page carries enough information
 * to render Previous / Next / First / Last navigation without
 * follow-up calls. `prevCursor` / `nextCursor` are opaque strings;
 * `isFirst` / `isLast` are explicit boundary flags (a single-page
 * result has both cursors null AND both flags true).
 */
export function pageSchema<T extends z.ZodTypeAny>(item: T) {
  return z.object({
    items: z.array(item),
    nextCursor: z.string().nullable(),
    prevCursor: z.string().nullable(),
    isFirst: z.boolean(),
    isLast: z.boolean(),
  });
}

// ── Concrete pages ────────────────────────────────────────────────────

export const PoolsPageSchema = pageSchema(PoolSchema);
export type PoolsPageResponse = z.infer<typeof PoolsPageSchema>;

export const SwapEventsPageSchema = pageSchema(SwapEventSchema);
export type SwapEventsPageResponse = z.infer<typeof SwapEventsPageSchema>;

export const LiquidityEventsPageSchema = pageSchema(LiquidityEventSchema);
export type LiquidityEventsPageResponse = z.infer<typeof LiquidityEventsPageSchema>;