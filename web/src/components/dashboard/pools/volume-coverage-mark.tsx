/**
 * The "this figure is a sub-total" mark — a compact `priced / total` suffix
 * next to a 24h volume, rendered only when the window was NOT fully valued.
 *
 * Why a suffix and not a column: the pool table already sits at its minimum
 * width (`TABLE_MIN_WIDTH_CLASS`, ~1030px) before it scrolls, and the Overview
 * ranking lives in a half-width grid cell. A tenth column would buy one number
 * at the cost of the whole table's density — and the number is only meaningful
 * on the minority of rows that carry it (3 pools out of 95 classable ones,
 * measured 7 August 2026).
 *
 * Silent at full coverage, by design. The KPI cards state the coverage even
 * when complete — there, an absent line would be ambiguous with "no data". In
 * a table of fifty rows the opposite is true: a mark on every row is noise, and
 * what the reader needs is to spot the few figures that do not mean what they
 * appear to mean.
 *
 * Pure presentation, no client JS: `title` and `aria-label` carry the sentence
 * the KPI cards spell out, so the fraction is never the only explanation.
 */

import type { VolumeCoverage } from "@/lib/coverage/volume-coverage";

import type { CoverageLabel } from "./pools-table-shared";

export function VolumeCoverageMark({
  coverage,
  labelFor,
}: {
  /** `null`, or a complete window — renders nothing in both cases. */
  coverage: VolumeCoverage | null;
  /** Resolves the localized sentence; called only when the mark is shown. */
  labelFor: CoverageLabel;
}) {
  if (coverage === null || !coverage.partial) {
    return null;
  }

  const label = labelFor(coverage.priced, coverage.total);

  return (
    <span
      className="ml-1.5 text-[11px] text-slate-500 tabular-nums"
      title={label}
      aria-label={label}
    >
      {coverage.priced}/{coverage.total}
    </span>
  );
}
