/**
 * The DAMM v2 page, a Server Component: its header, its KPIs in the protocol's scope, then the
 * row of its tabs. ⚠️ The KPIs are keyed by the period only: a tab change keeps them on screen.
 */

import { Suspense } from "react";
import { cookies } from "next/headers";
import type { Metadata } from "next";
import { getTranslations, setRequestLocale } from "next-intl/server";

import {
  KpiPlaceholder,
  PoolsKpi,
  TvlKpi,
  VolumeKpi,
} from "@/components/dashboard/overview/overview-kpis";
import { GRID_CLASS } from "@/components/dashboard/overview/overview-layout";
import { OverviewMode } from "@/components/dashboard/overview/overview-mode";
import { OVERVIEW_ADVANCED_COOKIE } from "@/components/dashboard/overview/overview-mode-state";
import { StaleBanner } from "@/components/dashboard/overview/overview-states";
import { FeesTab } from "@/components/dashboard/protocol/fees/fees-tab";
import { LiquidityTab } from "@/components/dashboard/protocol/liquidity/liquidity-tab";
import { PoolsTab } from "@/components/dashboard/protocol/pools/pools-tab";
import { ProtocolHeader } from "@/components/dashboard/protocol/protocol-header";
import { FeesRateKpi, ProtocolFeesKpi } from "@/components/dashboard/protocol/protocol-kpis";
import { ProtocolTabs } from "@/components/dashboard/protocol/protocol-tabs";
import { VolumeTab } from "@/components/dashboard/protocol/volume/volume-tab";
import { OVERVIEW_PERIODS } from "@/lib/overview/period";
import { readPeriod } from "@/lib/api/server/read-period";
import { parseTab, readyTabs } from "@/lib/protocol/tabs";
import { pageMetadata } from "@/lib/seo/page-metadata";

const PATHNAME = "/protocols/meteora-damm-v2";

const PROTOCOL = "meteora_damm_v2";

const PROGRAM_ID = "cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG";

type ProtocolPageProps = {
  params: Promise<{ locale: string }>;
  searchParams: Promise<{ period?: string | string[]; tab?: string | string[] }>;
};

export async function generateMetadata({ params }: ProtocolPageProps): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: "Dashboard.Protocol.page" });
  return pageMetadata({
    locale,
    path: PATHNAME,
    title: t("metaTitle"),
    description: t("description"),
  });
}

export default async function DammV2Page({ params, searchParams }: ProtocolPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);
  const query = await searchParams;
  const period = await readPeriod(query.period, OVERVIEW_PERIODS);
  const tab = parseTab(query.tab);
  const initialAdvanced = (await cookies()).get(OVERVIEW_ADVANCED_COOKIE)?.value === "1";
  const t = await getTranslations("Dashboard.Protocol");

  return (
    <OverviewMode initialAdvanced={initialAdvanced}>
      <ProtocolHeader
        pathname={PATHNAME}
        period={period}
        tab={tab}
        protocol={PROTOCOL}
        programId={PROGRAM_ID}
      />
      <Suspense fallback={null}>
        <StaleBanner period={period} />
      </Suspense>

      <div className={GRID_CLASS}>
        <Suspense key={`tvl-${period}`} fallback={<KpiPlaceholder />}>
          <TvlKpi period={period} scope={PROTOCOL} />
        </Suspense>
        <Suspense key={`volume-${period}`} fallback={<KpiPlaceholder />}>
          <VolumeKpi period={period} scope={PROTOCOL} />
        </Suspense>
        <Suspense key={`fees-${period}`} fallback={<KpiPlaceholder />}>
          <FeesRateKpi period={period} protocol={PROTOCOL} />
        </Suspense>
        <Suspense key={`protocol-fees-${period}`} fallback={<KpiPlaceholder advanced />}>
          <ProtocolFeesKpi period={period} protocol={PROTOCOL} />
        </Suspense>
        <Suspense key={`pools-${period}`} fallback={<KpiPlaceholder />}>
          <PoolsKpi period={period} scope={PROTOCOL} />
        </Suspense>
      </div>

      {tab !== null && (
        <ProtocolTabs
          pathname={PATHNAME}
          period={period}
          current={tab}
          tabs={readyTabs().map((key) => ({ key, label: t(`tabs.${key}`) }))}
          label={t("tabsLabel")}
        />
      )}
      {tab === "liquidity" && <LiquidityTab period={period} protocol={PROTOCOL} />}
      {tab === "volume" && <VolumeTab period={period} protocol={PROTOCOL} />}
      {tab === "fees" && <FeesTab period={period} protocol={PROTOCOL} />}
      {tab === "pools" && <PoolsTab period={period} protocol={PROTOCOL} />}
    </OverviewMode>
  );
}
