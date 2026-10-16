/** The watchlist page: a server shell around the client `WatchlistContent`. */

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
        {/* ⚠️ `description` is also the <meta description>: the caveat has its own key. */}
        <InfoPopover label={tShell("pageInfo")}>
          {t("description")} {t("coverageNote")}
        </InfoPopover>
      </header>

      <WatchlistContent />
    </div>
  );
}
