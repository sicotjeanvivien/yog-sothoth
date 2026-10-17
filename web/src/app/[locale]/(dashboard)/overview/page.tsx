/**
 * The Overview, a Server Component: the header renders at once, then the protocol KPI strip,
 * the top pools and the latest signals stream in, each in its own `<Suspense>` and each
 * degrading to a `BlockError` on its own.
 */

import { Suspense } from "react";
import type { Metadata } from "next";
import { getTranslations, setRequestLocale } from "next-intl/server";

import { BlockPlaceholder } from "@/components/dashboard/block-placeholder";
import { LiveIndicator } from "@/components/dashboard/live-status/live-indicator";
import { InfoPopover } from "@/components/shared/info-popover";
import { OverviewLatestSignals } from "@/components/dashboard/overview/overview-latest-signals";
import { OverviewStats } from "@/components/dashboard/overview/overview-stats";
import { OverviewTopPools } from "@/components/dashboard/overview/overview-top-pools";
import type { PoolRankMetric } from "@/lib/api/server/top-pools";

type OverviewPageProps = {
  params: Promise<{ locale: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
};

// Anything but `tvl` is the volume default: a bad URL must not break the page.
function parseRank(raw: string | string[] | undefined): PoolRankMetric {
  return raw === "tvl" ? "tvl" : "volume_24h";
}

export async function generateMetadata({
  params,
}: OverviewPageProps): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({
    locale,
    namespace: "Dashboard.Overview.page",
  });
  return {
    title: `${t("title")} — Yog-Scope`,
    description: t("description"),
  };
}

export default async function OverviewPage({
  params,
  searchParams,
}: OverviewPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);

  const sp = await searchParams;
  const rankMetric = parseRank(sp["rank"]);

  const t = await getTranslations("Dashboard.Overview.page");
  const tShell = await getTranslations("Dashboard.shell");

  return (
    <>
      <header className="px-6 pt-6 pb-4 font-dash-sans lg:px-10">
        <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
          <span className="font-dash-mono text-[11px] font-medium tracking-[0.12em] text-dash-ink-3 uppercase">
            {t("breadcrumb")}
          </span>
          <span className="-mr-[10px]">
            <LiveIndicator />
          </span>
        </div>
        <div className="mt-2 flex items-center gap-2.5">
          <h1 className="font-display text-[20px] leading-[1.2] font-bold tracking-[0.03em] text-[#f5f2ff]">
            {t("title")}
          </h1>
          <InfoPopover label={tShell("pageInfo")}>{t("description")}</InfoPopover>
        </div>
      </header>

      <Suspense
        fallback={
          <section className="px-6 lg:px-10">
            <BlockPlaceholder className="h-[124px]" />
          </section>
        }
      >
        <OverviewStats />
      </Suspense>

      <section className="mt-8 grid items-start gap-8 px-6 pb-10 lg:px-10 xl:grid-cols-2">
        <Suspense fallback={<BlockPlaceholder className="h-[520px]" />}>
          <OverviewTopPools metric={rankMetric} searchParams={sp} />
        </Suspense>
        <Suspense fallback={<BlockPlaceholder className="h-[520px]" />}>
          <OverviewLatestSignals />
        </Suspense>
      </section>
    </>
  );
}
