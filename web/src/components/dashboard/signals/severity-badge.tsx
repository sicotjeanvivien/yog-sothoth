/** A signal's severity as a word in a box, in the dashboard's rows. */

import type { SeverityResponse } from "@/lib/api/schema/zod.gen";

import { SEVERITY_BADGE } from "./signal-display";

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
