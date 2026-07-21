/**
 * Clickable ranking header for the Overview top-N pools table.
 *
 * Server Component — the click target is a next-intl `<Link>` with a
 * precomputed href, no client state (same approach as the pools
 * `SortableHeader`). Clicking a metric re-ranks the strip by writing
 * `?rank=<metric>` to the URL; the page re-renders server-side with the
 * new ranking.
 *
 * The ranking is always descending (biggest first) — there is no
 * asc/desc cycle. So the *active* header is deliberately NOT a link: it
 * renders as plain (brighter) text, because clicking it would re-rank by
 * what is already selected and do nothing — a no-op click that reads as
 * broken. Only the *inactive* metric is a clickable link. No direction
 * chevron either, for the same reason (it implies a toggle that isn't
 * there). `volume_24h` is the default ranking, so selecting it *drops*
 * the `rank` param to keep the URL clean (a bare `/overview` means "by
 * volume").
 */

import { getTranslations } from "next-intl/server";

import { Link } from "@/i18n/navigation";
import type { PoolRankMetric } from "@/lib/api/server/top-pools";

const DEFAULT_METRIC: PoolRankMetric = "volume_24h";

type OverviewRankHeaderProps = {
  /** Metric this header ranks by when clicked. */
  metric: PoolRankMetric;
  /** Visible label. */
  label: string;
  /** Currently active ranking, resolved from the URL. */
  activeMetric: PoolRankMetric;
  /** Current search params — unrelated ones are preserved. */
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

  // Active header: plain text, not a link — nothing to click, so no
  // misleading no-op. `aria-current` marks it as the current ranking.
  if (isActive) {
    return (
      <span aria-current="true" className={`${baseClass} text-slate-200`}>
        {label}
      </span>
    );
  }

  // Inactive metric: the clickable switch to that ranking.
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

/**
 * Build the target href: keep every unrelated param, set `rank` — or drop
 * it when selecting the default (volume) so the URL stays bare.
 */
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
