/** A signal severity of the home page's illustrations: filled to look at, outlined for info. */

import { useTranslations } from "next-intl";

import { SEVERITY_BADGE } from "@/components/dashboard/signals/signal-display";
import type { SeverityResponse } from "@/lib/api/schema/zod.gen";

export function SeverityBadge({ severity }: { severity: SeverityResponse }) {
  const t = useTranslations("Dashboard.Signals.feed.severity");

  return (
    <span
      className={`inline-block min-w-16 px-1 py-0.5 text-center font-dash-mono text-[11px] leading-none font-semibold tracking-[0.08em] uppercase ${SEVERITY_BADGE[severity]}`}
    >
      {t(severity)}
    </span>
  );
}
