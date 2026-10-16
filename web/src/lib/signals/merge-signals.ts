/**
 * The live feed's one merge, for SSE events and post-reconnect refills alike: dedup by `id`,
 * display order, cap.
 */

import type { SignalResponse } from "@/lib/api/schema/zod.gen";

/** A live tail, not a history: older entries fall off, the paginated API browses back. */
export const FEED_CAP = 200;

/** Union by `id`, newest first (`triggeredAt` desc, `id` desc), truncated to `cap`. */
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
