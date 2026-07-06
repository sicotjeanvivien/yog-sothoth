/**
 * Local filtering of the signal feed — pure, framework-free.
 *
 * Selection model (mirrors the chips UI):
 *   - within a dimension, active values are OR'd;
 *   - across dimensions (severity × detector), filters are AND'd;
 *   - an EMPTY selection means "no filter" for that dimension — the
 *     default state of an alert feed must never hide anything.
 *
 * Applied at render time on the merged live list: the SSE stream and
 * the 200-item merge are untouched, hidden signals stay in memory and
 * reappear the moment the filter releases them.
 */

import type { Severity, SignalResponse } from "@/lib/api/schema/signal";

export function filterSignals(
  signals: readonly SignalResponse[],
  activeSeverities: ReadonlySet<Severity>,
  activeDetectors: ReadonlySet<string>,
): readonly SignalResponse[] {
  if (activeSeverities.size === 0 && activeDetectors.size === 0) {
    return signals;
  }
  return signals.filter(
    (signal) =>
      (activeSeverities.size === 0 || activeSeverities.has(signal.severity)) &&
      (activeDetectors.size === 0 || activeDetectors.has(signal.detector)),
  );
}
