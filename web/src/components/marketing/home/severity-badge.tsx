/** A signal severity of the home page's illustrations: filled to look at, outlined for info. */

import { useTranslations } from "next-intl";

import type { SeverityResponse } from "@/lib/api/schema/zod.gen";

const SEVERITY_CLASS: Record<SeverityResponse, string> = {
  critical: "bg-dash-crit text-dash-surface",
  warning: "bg-dash-warn text-dash-surface",
  info: "text-dash-ink-2 shadow-[inset_0_0_0_1px_var(--color-dash-ink-2)]",
};

export function SeverityBadge({ severity }: { severity: SeverityResponse }) {
  const t = useTranslations("Dashboard.Signals.feed.severity");

  return (
    <span
      className={`inline-block min-w-16 px-1 py-0.5 text-center font-dash-mono text-[11px] leading-none font-semibold tracking-[0.08em] uppercase ${SEVERITY_CLASS[severity]}`}
    >
      {t(severity)}
    </span>
  );
}
