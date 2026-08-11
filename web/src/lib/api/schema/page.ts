import * as z from "zod";
import { PoolSchema } from "./pool";
import { SwapEventSchema } from "./swap-event";
import { LiquidityEventSchema } from "./liquidity-event";
import { SignalSchema } from "./signal";

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

/**
 * `GET /api/pools` — the shared envelope plus what this listing alone says
 * about its traversal.
 *
 * `last_seen_at` is rewritten on every event touching a pool, so a listing
 * sorted on it is anchored to `asOf` (the instant the traversal started) and
 * reads only pools at or below it. A pool that becomes active after `asOf`
 * moves to the head of the live list — past a reader who is already deeper in
 * it — so this traversal cannot show it. `touchedSince` is how many did,
 * which is what lets the UI say so instead of quietly dropping them.
 *
 * `asOf` is `null` when the sort is over an immutable column
 * (`first_seen_*`), which needs no anchor.
 */
export const PoolsPageSchema = pageSchema(PoolSchema).extend({
  asOf: z.string().nullable(),
  touchedSince: z.number().int().nonnegative(),
});
export type PoolsPageResponse = z.infer<typeof PoolsPageSchema>;

export const SwapEventsPageSchema = pageSchema(SwapEventSchema);
export type SwapEventsPageResponse = z.infer<typeof SwapEventsPageSchema>;

export const LiquidityEventsPageSchema = pageSchema(LiquidityEventSchema);
export type LiquidityEventsPageResponse = z.infer<typeof LiquidityEventsPageSchema>;

export const SignalsPageSchema = pageSchema(SignalSchema);
export type SignalsPageResponse = z.infer<typeof SignalsPageSchema>;