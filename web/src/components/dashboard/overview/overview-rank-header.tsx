/**
 * The Overview ranking header, a Server Component of precomputed `<Link>`s. Always descending,
 * so the active metric is plain text, not a no-op link, with no chevron. Selecting the default,
 * `volume_24h`, drops `rank` from the URL.
 */

import { getTranslations } from "next-intl/server";

import { Link } from "@/i18n/navigation";
import type { PoolRankMetric } from "@/lib/api/server/top-pools";

const DEFAULT_METRIC: PoolRankMetric = "volume_24h";

type OverviewRankHeaderProps = {
  metric: PoolRankMetric;
  label: string;
  activeMetric: PoolRankMetric;
  /** Unrelated params are preserved. */
  searchParams: Record<string, string | string[] | undefined>;
};

export async function OverviewRankHeader({
  metric,
  label,
  activeMetric,
  searchParams,
}: OverviewRankHeaderProps) {
  const t = await getTranslations("Dashboard.Overview.topPools");

  const isActive = metric === activeMetric;

  const baseClass =
    "inline-flex items-center justify-end text-[12px] font-semibold tracking-[0.2em] uppercase whitespace-nowrap";

  if (isActive) {
    return (
      <span aria-current="true" className={`${baseClass} text-slate-200`}>
        {label}
      </span>
    );
  }

  return (
    <Link
      href={buildHref(searchParams, metric)}
      aria-label={t("rankBy", { metric: label })}
      className={`
        ${baseClass} text-slate-500 transition-colors hover:text-slate-300
        focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sothoth-400
        rounded
      `}
    >
      {label}
    </Link>
  );
}

function buildHref(
  searchParams: Record<string, string | string[] | undefined>,
  metric: PoolRankMetric,
): string {
  const next = new URLSearchParams();

  for (const [key, value] of Object.entries(searchParams)) {
    if (key === "rank") continue;
    if (value === undefined) continue;
    if (Array.isArray(value)) {
      for (const v of value) next.append(key, v);
    } else {
      next.set(key, value);
    }
  }

  if (metric !== DEFAULT_METRIC) {
    next.set("rank", metric);
  }

  const qs = next.toString();
  return qs.length > 0 ? `/overview?${qs}` : "/overview";
}
