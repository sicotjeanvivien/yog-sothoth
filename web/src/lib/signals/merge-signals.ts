/**
 * Pure data-shaping for the live signal feed.
 *
 * The feed's in-memory list is fed from two directions — SSE events
 * prepend one by one, and a post-reconnect refill brings a whole page —
 * so both paths funnel through one merge: dedup by `id`, re-sort to the
 * feed's display order, cap the length. Kept free of React so the
 * reconciliation logic is unit-testable on its own.
 */

import type { SignalResponse } from "@/lib/api/schema/signal";

/**
 * In-memory cap of the feed. The page is a live tail, not an infinite
 * history — older entries fall off; the paginated API remains the way
 * to browse back.
 */
export const FEED_CAP = 200;

/**
 * Merge `incoming` signals into `current`: union by `id` (incoming
 * wins — signals are immutable so both copies are identical anyway),
 * sorted newest first (`triggeredAt` desc, `id` desc — the feed's
 * display order), truncated to `cap`.
 */
export function mergeSignals(
  current: readonly SignalResponse[],
  incoming: readonly SignalResponse[],
  cap: number = FEED_CAP,
): SignalResponse[] {
  const byId = new Map<number, SignalResponse>();
  for (const signal of current) {
    byId.set(signal.id, signal);
  }
  for (const signal of incoming) {
    byId.set(signal.id, signal);
  }

  return [...byId.values()]
    .sort(
      (a, b) =>
        Date.parse(b.triggeredAt) - Date.parse(a.triggeredAt) || b.id - a.id,
    )
    .slice(0, cap);
}
