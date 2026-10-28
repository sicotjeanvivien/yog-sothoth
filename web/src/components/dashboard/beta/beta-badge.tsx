import { useTranslations } from "next-intl";

export function BetaBadge({ className = "" }: { className?: string }) {
  const t = useTranslations("Dashboard.Beta");

  return (
    <span
      title={t("badgeTitle")}
      className={`border border-dash-accent/45 px-1.5 font-dash-mono text-[11px] leading-4 font-medium tracking-[0.08em] text-dash-accent uppercase ${className}`}
    >
      {t("badge")}
    </span>
  );
}
