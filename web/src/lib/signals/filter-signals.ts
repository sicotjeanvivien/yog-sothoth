/**
 * The feed's local filters: OR within a dimension, AND across. ⚠️ An empty selection is no
 * filter: an alert feed's default must never hide anything.
 */

import type { SeverityResponse, SignalResponse } from "@/lib/api/schema/zod.gen";

export function filterSignals(
  signals: readonly SignalResponse[],
  activeSeverities: ReadonlySet<SeverityResponse>,
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
