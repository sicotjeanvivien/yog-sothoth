/**
 * Overview page — protocol-wide KPI strip.
 *
 * Four scalar cards, stacked on mobile, from `GET /api/stats`:
 *
 *   - Total TVL   (+ coverage hint "N / M priced")
 *   - Volume 24h  (realized, trade-time valued)
 *   - Fees 24h    (realized trading fee revenue) — the differentiator
 *   - Pools       (observed; + discovery hint "+K discovered (24h)")
 *
 * USD values render `—` when null (nothing priceable / no activity in
 * the window) — the format helpers own the null check. The coverage and
 * discovery hints are composed here from the raw counters the API ships,
 * keeping presentation out of the endpoint.
 */

import { getTranslations } from "next-intl/server";

import type { StatsResponse } from "@/lib/api/schema/stats";
import { formatCount } from "@/lib/format/format-count";
import { formatUsdCompact } from "@/lib/format/format-usd";

import { StatCard } from "./stat-card";

export async function OverviewStats({ stats }: { stats: StatsResponse }) {
  const t = await getTranslations("Dashboard.Overview.kpis");

  return (
    <section className="px-6 lg:px-10">
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard
          label={t("tvl")}
          value={formatUsdCompact(stats.totalTvlUsd)}
          hint={t("coverage", {
            priced: stats.poolsPriced,
            observed: stats.poolsObserved,
          })}
          info={t("info.tvl")}
        />
        <StatCard
          label={t("volume24h")}
          value={formatUsdCompact(stats.volume24hUsd)}
          info={t("info.volume24h")}
        />
        <StatCard
          label={t("fees24h")}
          value={formatUsdCompact(stats.fees24hUsd)}
          info={t("info.fees24h")}
        />
        <StatCard
          label={t("pools")}
          value={formatCount(stats.poolsObserved)}
          hint={t("discovered", { count: stats.poolsDiscovered24h })}
          info={t("info.pools")}
        />
      </div>
    </section>
  );
}
