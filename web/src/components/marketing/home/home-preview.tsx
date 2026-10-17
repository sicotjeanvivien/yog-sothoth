/** The dashboard preview of the hero, an illustration with fixed figures. */

import { useTranslations } from "next-intl";

import { Link } from "@/i18n/navigation";
import { ArrowRightIcon } from "@/components/shared/icon";
import { SITE_LABEL_CLASS } from "@/components/marketing/site-label";
import type { SeverityResponse } from "@/lib/api/schema/zod.gen";
import { SeverityBadge } from "./severity-badge";

const KPI_KEYS = ["tvl", "volume", "fees", "pools"] as const;

// 24 hours, in px.
const BAR_HEIGHTS = [
  39, 37, 47, 39, 40, 31, 35, 22, 34, 35, 23, 36, 25, 33, 42, 39, 44, 48, 49,
  54, 43, 51, 42, 40,
] as const;

const SIGNALS: readonly { key: string; severity: SeverityResponse; time: string }[] = [
  { key: "drain", severity: "critical", time: "21:14" },
  { key: "imbalance", severity: "warning", time: "20:37" },
  { key: "oracle", severity: "info", time: "16:03" },
];

export function HomePreview() {
  const t = useTranslations("Marketing.preview");

  return (
    <>
      <div
        role="img"
        aria-label={t("label")}
        className="border border-dash-rule bg-dash-surface"
      >
        <div className="flex flex-wrap items-center justify-between gap-2 border-b border-dash-rule px-4 py-3 text-[12px] text-dash-ink-2">
          <span>{t("title")}</span>
          <span className="font-dash-mono">{t("tag")}</span>
        </div>

        <dl className="grid grid-cols-2">
          {KPI_KEYS.map((key) => (
            <div
              key={key}
              className="flex flex-col gap-1 border-b border-dash-rule px-4 py-3.5 odd:border-r"
            >
              <dt className={SITE_LABEL_CLASS}>{t(`kpis.${key}.label`)}</dt>
              <dd className="font-dash-mono text-[22px] font-medium tracking-[-0.02em] tabular-nums">
                {t(`kpis.${key}.value`)}
              </dd>
            </div>
          ))}
        </dl>

        <div className="flex h-18 items-end gap-0.5 border-b border-dash-rule px-4 pt-4">
          {BAR_HEIGHTS.map((height, hour) => (
            <div
              key={hour}
              className="min-w-0 flex-1 rounded-t-[2px] bg-dash-accent"
              style={{ height }}
            />
          ))}
        </div>

        <ul>
          {SIGNALS.map((signal) => (
            <li
              key={signal.key}
              className="grid grid-cols-[72px_40px_minmax(0,1fr)] items-baseline gap-2.5 border-b border-dash-rule px-4 py-2.25 text-[12px] last:border-b-0"
            >
              <SeverityBadge severity={signal.severity} />
              <span className="font-dash-mono text-dash-ink-3">{signal.time}</span>
              <span>{t(`signals.${signal.key}`)}</span>
            </li>
          ))}
        </ul>
      </div>

      <Link
        href="/overview"
        className="flex min-h-11 items-center justify-between gap-3 border border-t-0 border-dash-rule bg-dash-surface px-4 text-[14px] text-dash-ink-2 transition-colors hover:bg-dash-surface-2 hover:text-dash-ink"
      >
        {t("seeAll")}
        <ArrowRightIcon size={14} />
      </Link>
    </>
  );
}
