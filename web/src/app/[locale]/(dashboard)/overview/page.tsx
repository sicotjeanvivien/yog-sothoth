/**
 * Overview page (`/[locale]/(dashboard)/overview`).
 *
 * Server Component. Calls `fetchStats` directly from `lib/api` (the call
 * already runs on the Next.js server, no BFF round-trip needed) and renders
 * the protocol-wide KPI strip.
 *
 * Two display states:
 *   - error → `PageError` (driven by `ApiClientError.details.kind`)
 *   - ok    → header + `OverviewStats` (the 4-card strip)
 *
 * No ingestion-health hero here on purpose: it already lives everywhere in
 * the dashboard chrome via the sidebar `network-status-panel`. Below the
 * KPI strip, two self-degrading blocks side by side: the top pools ranked by
 * volume or TVL (`?rank=`) and the 5 latest signals (each fetches its own
 * data and falls back to a `BlockError` without taking the page down).
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

// The top-N ranking metric comes from `?rank=`. Anything other than the
// explicit `tvl` (including absent or a stale/edited value) falls back to the
// volume default — a bad URL shouldn't break the page.
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

      {/* Two independent self-degrading blocks, side by side on wide
          screens: the pool ranking (volume/TVL) and the latest alerts. */}
      <section className="mt-8 grid items-start gap-8 px-6 pb-10 lg:px-10 xl:grid-cols-2">
        <OverviewTopPools metric={rankMetric} searchParams={sp} />
        <OverviewLatestSignals />
      </section>
    </>
  );
}
