/**
 * Watchlist page (`/[locale]/(dashboard)/watchlist`).
 *
 * Server Component shell (title + ⓘ) around the client `WatchlistContent`,
 * which owns the LocalStorage read and the per-pool fetches. The watchlist
 * lives entirely in the browser — no account, no backend — until the v0.3
 * server-side watchlist replaces the store behind the same hook.
 */

import type { Metadata } from "next";
import { getTranslations, setRequestLocale } from "next-intl/server";

import { WatchlistContent } from "@/components/dashboard/watchlist/watchlist-content";
import { InfoPopover } from "@/components/shared/info-popover";

type WatchlistPageProps = {
  params: Promise<{ locale: string }>;
};

export async function generateMetadata({
  params,
}: WatchlistPageProps): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({
    locale,
    namespace: "Dashboard.Watchlist.page",
  });
  return {
    title: `${t("title")} — Yog-Scope`,
    description: t("description"),
  };
}

export default async function WatchlistPage({ params }: WatchlistPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);

  const t = await getTranslations("Dashboard.Watchlist.page");
  const tShell = await getTranslations("Dashboard.shell");

  return (
    <div className="pb-16">
      <header className="flex items-center gap-2.5 px-6 pt-6 pb-4 lg:px-10">
        <h1 className="font-display text-[20px] leading-[1.2] font-bold tracking-[0.03em] text-[#f5f2ff]">
          {t("title")}
        </h1>
        <InfoPopover label={tShell("pageInfo")}>{t("description")}</InfoPopover>
      </header>

      <WatchlistContent />
    </div>
  );
}
