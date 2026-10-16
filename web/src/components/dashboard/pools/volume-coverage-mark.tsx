/**
 * A `priced / total` suffix on a 24h volume that was not fully valued, silent otherwise: in a
 * table, a mark on every row is noise. ⚠️ Its sentence is a visually-hidden sibling — not
 * `aria-label`, forbidden on a bare `<span>`, nor `title`, which a screen reader would
 * announce a second time.
 */

import type { VolumeCoverage } from "@/lib/coverage/volume-coverage";

import type { CoverageLabel } from "./pools-table-shared";

export function VolumeCoverageMark({
  coverage,
  labelFor,
}: {
  /** `null`, or a complete window — renders nothing in both cases. */
  coverage: VolumeCoverage | null;
  labelFor: CoverageLabel;
}) {
  if (coverage === null || !coverage.partial) {
    return null;
  }

  const label = labelFor(coverage.priced, coverage.total);

  return (
    <span className="ml-1.5 text-[11px] text-slate-500 tabular-nums">
      <span aria-hidden="true">
        {coverage.priced}/{coverage.total}
      </span>
      <span className="sr-only">{label}</span>
    </span>
  );
}
