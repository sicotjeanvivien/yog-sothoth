/**
 * The live feed's rows: the signals, and a gap row at each reopening of the stream, which carries
 * no history. All sorted together by time, so a gap row says what came before and after it.
 */

import type { SignalResponse } from "@/lib/api/schema/zod.gen";

/** A live tail, not a history: older entries fall off, the paginated API browses back. */
export const FEED_CAP = 200;

export type SignalFeedRow =
  | { type: "signal"; key: string; signal: SignalResponse }
  | { type: "gap"; key: string; at: string };

/** The rows with `incoming` merged by `id`, newest first (time desc, `id` desc), cut to `cap`. */
export function mergeSignals(
  rows: readonly SignalFeedRow[],
  incoming: readonly SignalResponse[],
  cap: number = FEED_CAP,
): SignalFeedRow[] {
  const byKey = new Map<string, SignalFeedRow>();
  for (const row of rows) {
    byKey.set(row.key, row);
  }
  for (const signal of incoming) {
    byKey.set(signalKey(signal.id), { type: "signal", key: signalKey(signal.id), signal });
  }
  return sorted([...byKey.values()]).slice(0, cap);
}

/** A gap at `now`, only over signals already shown and never with no signal since the last one. */
export function markSignalGap(rows: readonly SignalFeedRow[], now: string): readonly SignalFeedRow[] {
  if (!rows.some((row) => row.type === "signal")) {
    return rows;
  }
  // ⚠️ `now` is the browser's clock, the signals the server's: a late clock must not sink the gap
  // below a signal shown before the cut, which a tie would rank above it.
  const at = new Date(Math.max(Date.parse(now), ...rows.map((row) => timeOf(row) + 1))).toISOString();
  const gap: SignalFeedRow = { type: "gap", key: `gap:${at}`, at };
  const next = sorted([...rows, gap]);
  const below = next[next.indexOf(gap) + 1];
  return below?.type === "gap" ? rows : next.slice(0, FEED_CAP);
}

function signalKey(id: number): string {
  return `signal:${id}`;
}

function timeOf(row: SignalFeedRow): number {
  return Date.parse(row.type === "signal" ? row.signal.triggeredAt : row.at);
}

/** ⚠️ At the same instant a signal sorts above a gap: it was triggered at the reopening, not before. */
function sorted(rows: SignalFeedRow[]): SignalFeedRow[] {
  return rows.sort((a, b) => {
    const byTime = timeOf(b) - timeOf(a);
    if (byTime !== 0) return byTime;
    if (a.type !== b.type) return a.type === "signal" ? -1 : 1;
    return a.type === "signal" && b.type === "signal" ? b.signal.id - a.signal.id : 0;
  });
}
