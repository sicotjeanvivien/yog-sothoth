/**
 * The Overview's KPI strip from `GET /api/stats`, with the coverage and discovery hints
 * composed from the raw counters. Fees share the volume's valuation, so they carry no hint.
 */

import { getTranslations } from "next-intl/server";

import type { StatsResponse } from "@/lib/api/schema/zod.gen";
import { formatCount } from "@/lib/format/format-count";
import { formatUsdCompact } from "@/lib/format/format-usd";
import { volumeCoverage } from "@/lib/coverage/volume-coverage";

import { StatCard } from "./stat-card";

export async function OverviewStats({ stats }: { stats: StatsResponse }) {
  const t = await getTranslations("Dashboard.Overview.kpis");

  // Over the global counters, pool-hours across all pools.
  const coverage = volumeCoverage(stats);

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
          hint={
            coverage !== null
              ? t("bucketCoverage", {
                priced: coverage.priced,
                total: coverage.total,
              })
              : undefined
          }
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
