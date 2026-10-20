/**
 * The share valued, "97 %", after a volume that was not fully valued, silent otherwise: in a
 * table, a mark on every row is noise. ⚠️ Its sentence is a visually-hidden sibling — not
 * `aria-label`, forbidden on a bare `<span>`, nor `title`, which a screen reader would
 * announce a second time.
 */

import { getLocale } from "next-intl/server";

import type { VolumeCoverage } from "@/lib/coverage/volume-coverage";
import { formatPercent } from "@/lib/format/format-percent";

export async function VolumeCoverageMark({
  coverage,
  label,
}: {
  /** `null`, or a complete window — renders nothing in both cases. */
  coverage: VolumeCoverage | null;
  label: (priced: number, total: number) => string;
}) {
  if (coverage === null || !coverage.partial) {
    return null;
  }
  const locale = await getLocale();

  return (
    <span className="ml-1.5 text-[11px] text-dash-ink-3 tabular-nums">
      <span aria-hidden="true">
        {formatPercent(String(coverage.share), locale)}
      </span>
      <span className="sr-only">{label(coverage.priced, coverage.total)}</span>
    </span>
  );
}
