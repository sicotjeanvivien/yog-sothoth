/** A signal severity of the home page's illustrations: filled to look at, outlined for info. */

import { useTranslations } from "next-intl";

export type Severity = "critical" | "warning" | "info";

const SEVERITY_CLASS: Record<Severity, string> = {
  critical: "bg-dash-crit text-dash-surface",
  warning: "bg-dash-warn text-dash-surface",
  info: "text-dash-ink-2 shadow-[inset_0_0_0_1px_var(--color-dash-ink-2)]",
};

export function SeverityBadge({ severity }: { severity: Severity }) {
  const t = useTranslations("Marketing.severity");

  return (
    <span
      className={`inline-block min-w-16 px-1 py-0.5 text-center font-dash-mono text-[11px] leading-none font-semibold tracking-[0.08em] uppercase ${SEVERITY_CLASS[severity]}`}
    >
      {t(severity)}
    </span>
  );
}
