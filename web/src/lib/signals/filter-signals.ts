/**
 * The feed's local filters: OR within a dimension, AND across. ⚠️ An empty selection is no
 * filter: an alert feed's default must never hide anything.
 */

import type { SeverityResponse } from "@/lib/api/schema/zod.gen";

import type { SignalFeedRow } from "./merge-signals";

/** ⚠️ A gap row passes every filter: a filtered feed still shows where the stream was cut. */
export function filterSignals(
  rows: readonly SignalFeedRow[],
  activeSeverities: ReadonlySet<SeverityResponse>,
  activeDetectors: ReadonlySet<string>,
): readonly SignalFeedRow[] {
  if (activeSeverities.size === 0 && activeDetectors.size === 0) {
    return rows;
  }
  return rows.filter(
    (row) =>
      row.type === "gap" ||
      ((activeSeverities.size === 0 || activeSeverities.has(row.signal.severity)) &&
        (activeDetectors.size === 0 || activeDetectors.has(row.signal.detector))),
  );
}
