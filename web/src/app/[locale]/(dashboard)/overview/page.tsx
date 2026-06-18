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
 * the dashboard chrome via the sidebar `network-status-panel`. Top-N pools
 * is out of scope for phase 1 (see BACKLOG → Overview phase 1.5).
 */

import type { Metadata } from "next";
import { getTranslations, setRequestLocale } from "next-intl/server";

import { OverviewStats } from "@/components/dashboard/overview/overview-stats";
import { PageError } from "@/components/dashboard/page-error";
import { ApiClientError } from "@/lib/api/errors";
import { fetchStats } from "@/lib/api/server/stats";

type OverviewPageProps = {
  params: Promise<{ locale: string }>;
};

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

export default async function OverviewPage({ params }: OverviewPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);

  const t = await getTranslations("Dashboard.Overview.page");

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
      <header className="px-6 pt-8 pb-6 lg:px-10 lg:pt-10">
        <p className="text-[12px] font-semibold tracking-[0.28em] text-slate-400 uppercase">
          {t("eyebrow")}
        </p>
        <h1 className="mt-2 font-display text-[28px] leading-[1.15] font-bold tracking-[0.03em] text-[#f5f2ff] lg:text-[34px]">
          {t("title")}
        </h1>
        <p className="mt-3 max-w-[68ch] text-[15px] leading-[1.6] text-slate-400">
          {t("description")}
        </p>
      </header>

      <OverviewStats stats={stats} />
    </>
  );
}
