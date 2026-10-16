/**
 * The Overview, a Server Component: the protocol KPI strip, then the top pools and the latest
 * signals, each fetching its own data and degrading to a `BlockError` on its own.
 */

import type { Metadata } from "next";
import { getTranslations, setRequestLocale } from "next-intl/server";

import { InfoPopover } from "@/components/shared/info-popover";
import { OverviewLatestSignals } from "@/components/dashboard/overview/overview-latest-signals";
import { OverviewStats } from "@/components/dashboard/overview/overview-stats";
import { OverviewTopPools } from "@/components/dashboard/overview/overview-top-pools";
import { PageError } from "@/components/dashboard/page-error";
import { ApiClientError } from "@/lib/api/errors";
import { fetchStats } from "@/lib/api/server/stats";
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

  let stats;
  try {
    stats = await fetchStats();
  } catch (err) {
    if (err instanceof ApiClientError) {
      return <PageError kind={err.details.kind} />;
    }
    throw err;
  }

  return (
    <>
      <header className="flex items-center gap-2.5 px-6 pt-6 pb-4 lg:px-10">
        <h1 className="font-display text-[20px] leading-[1.2] font-bold tracking-[0.03em] text-[#f5f2ff]">
          {t("title")}
        </h1>
        <InfoPopover label={tShell("pageInfo")}>{t("description")}</InfoPopover>
      </header>

      <OverviewStats stats={stats} />

      <section className="mt-8 grid items-start gap-8 px-6 pb-10 lg:px-10 xl:grid-cols-2">
        <OverviewTopPools metric={rankMetric} searchParams={sp} />
        <OverviewLatestSignals />
      </section>
    </>
  );
}
