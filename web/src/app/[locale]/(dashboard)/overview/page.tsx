/**
 * The Overview, a Server Component: the header renders at once, then every block streams in
 * its own `<Suspense>` and degrades on its own. ⚠️ The blocks are keyed by the period, so a
 * period change shows their waiting slots while the header and its controls stay.
 */

import { Suspense } from "react";
import { cookies } from "next/headers";
import type { Metadata } from "next";
import { getTranslations, setRequestLocale } from "next-intl/server";

import { OverviewHeader } from "@/components/dashboard/overview/overview-header";
import {
  FeesKpi,
  KpiPlaceholder,
  PoolsKpi,
  RotationKpi,
  SwapsKpi,
  TvlKpi,
  VolumeKpi,
} from "@/components/dashboard/overview/overview-kpis";
import { GRID_CLASS } from "@/components/dashboard/overview/overview-layout";
import { OverviewMode } from "@/components/dashboard/overview/overview-mode";
import { OVERVIEW_ADVANCED_COOKIE } from "@/components/dashboard/overview/overview-mode-state";
import { StaleBanner } from "@/components/dashboard/overview/overview-states";
import {
  OverviewSignals,
  OverviewSignalsPlaceholder,
} from "@/components/dashboard/overview/overview-signals";
import {
  ProtocolActivity,
  ProtocolActivityPlaceholder,
} from "@/components/dashboard/overview/protocol-activity";
import {
  TopTokens,
  TopTokensPlaceholder,
} from "@/components/dashboard/overview/top-tokens";
import {
  VolumeChart,
  VolumeChartPlaceholder,
} from "@/components/dashboard/overview/volume-chart";
import { OVERVIEW_PERIODS } from "@/lib/overview/period";
import { readPeriod } from "@/lib/api/server/read-period";
import { pageMetadata } from "@/lib/seo/page-metadata";

type OverviewPageProps = {
  params: Promise<{ locale: string }>;
  searchParams: Promise<{ period?: string | string[] }>;
};

export async function generateMetadata({
  params,
}: OverviewPageProps): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({
    locale,
    namespace: "Dashboard.Overview.page",
  });
  return pageMetadata({
    locale,
    path: "/overview",
    title: t("title"),
    description: t("description"),
  });
}

export default async function OverviewPage({
  params,
  searchParams,
}: OverviewPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);
  const period = await readPeriod((await searchParams).period, OVERVIEW_PERIODS);
  const initialAdvanced = (await cookies()).get(OVERVIEW_ADVANCED_COOKIE)?.value === "1";

  return (
    <OverviewMode initialAdvanced={initialAdvanced}>
      <OverviewHeader period={period} />
      <Suspense fallback={null}>
        <StaleBanner period={period} />
      </Suspense>

      <div className={GRID_CLASS}>
        <Suspense key={`tvl-${period}`} fallback={<KpiPlaceholder />}>
          <TvlKpi period={period} scope="all" />
        </Suspense>
        <Suspense key={`volume-${period}`} fallback={<KpiPlaceholder />}>
          <VolumeKpi period={period} scope="all" />
        </Suspense>
        <Suspense key={`fees-${period}`} fallback={<KpiPlaceholder />}>
          <FeesKpi period={period} scope="all" />
        </Suspense>
        <Suspense key={`swaps-${period}`} fallback={<KpiPlaceholder advanced />}>
          <SwapsKpi period={period} scope="all" />
        </Suspense>
        <Suspense key={`pools-${period}`} fallback={<KpiPlaceholder />}>
          <PoolsKpi period={period} scope="all" />
        </Suspense>
        <Suspense key={`rotation-${period}`} fallback={<KpiPlaceholder advanced />}>
          <RotationKpi period={period} scope="all" />
        </Suspense>
      </div>

      <div className={GRID_CLASS}>
        <Suspense key={`protocols-${period}`} fallback={<ProtocolActivityPlaceholder period={period} />}>
          <ProtocolActivity period={period} />
        </Suspense>
        <Suspense fallback={<OverviewSignalsPlaceholder />}>
          <OverviewSignals />
        </Suspense>
        <Suspense key={`chart-${period}`} fallback={<VolumeChartPlaceholder period={period} />}>
          <VolumeChart period={period} />
        </Suspense>
        <Suspense key={`tokens-${period}`} fallback={<TopTokensPlaceholder period={period} />}>
          <TopTokens period={period} />
        </Suspense>
      </div>
    </OverviewMode>
  );
}
