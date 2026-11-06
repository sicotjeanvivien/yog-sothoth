/**
 * The gaps of the live signal feed: a reopening replays nothing, so the feed says where it
 * resumed rather than look whole. A gap sits at its instant among the signals, sorted by time.
 */

import type { SignalResponse } from "@/lib/api/schema/zod.gen";

export type SignalRow =
  | { type: "signal"; signal: SignalResponse }
  | { type: "gap"; key: string; at: string };

/** `signals` (newest first) with each gap of `gaps` at its instant: only below a newer signal or
 *  above an older one, so a gap over nothing shown is left out, and never two in a row. */
export function withGaps(
  signals: readonly SignalResponse[],
  gaps: readonly string[],
): SignalRow[] {
  const pending = [...gaps].sort((a, b) => Date.parse(b) - Date.parse(a));
  const rows: SignalRow[] = [];
  for (const signal of signals) {
    const at = Date.parse(signal.triggeredAt);
    let gap: string | undefined;
    while (pending.length > 0 && Date.parse(pending[0]!) > at) {
      gap ??= pending[0];
      pending.shift();
    }
    if (gap !== undefined && rows.at(-1)?.type !== "gap") {
      rows.push({ type: "gap", key: `gap:${gap}`, at: gap });
    }
    rows.push({ type: "signal", signal });
  }
  return rows;
}
