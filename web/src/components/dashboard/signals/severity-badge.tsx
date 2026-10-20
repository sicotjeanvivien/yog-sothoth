/**
 * A signal's severity as a word in a box: filled to be looked at (Critical, Warning),
 * outlined for information. ⚠️ Dark text on the fills: white reads at 2.7:1 on them.
 */

import type { SeverityResponse } from "@/lib/api/schema/zod.gen";

const SEVERITY_BADGE: Record<SeverityResponse, string> = {
  critical: "bg-dash-down text-dash-surface",
  warning: "bg-dash-warn text-dash-surface",
  info: "text-dash-ink-2 shadow-[inset_0_0_0_1px_var(--color-dash-ink-2)]",
};

export function SeverityBadge({
  severity,
  label,
}: {
  severity: SeverityResponse;
  label: string;
}) {
  return (
    <span
      className={`inline-block min-w-16 px-1.5 py-[3px] text-center font-dash-mono text-[11px] font-semibold tracking-[0.08em] uppercase ${SEVERITY_BADGE[severity]}`}
    >
      {label}
    </span>
  );
}
