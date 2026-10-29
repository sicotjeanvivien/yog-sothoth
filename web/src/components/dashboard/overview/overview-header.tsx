/** The compact header: the title alone; at the right, "Live" over the period and the mode. */

import { getTranslations } from "next-intl/server";

import { OVERVIEW_PERIODS, type Period } from "@/lib/overview/period";

import { HeaderControls } from "./header-controls";

export async function OverviewHeader({ period }: { period: Period }) {
  const t = await getTranslations("Dashboard.Overview.page");

  return (
    <header className="flex flex-wrap items-start justify-between gap-x-8 gap-y-3 border-b border-dash-rule px-4 py-5 font-dash-sans rail:px-8 rail:pt-4 rail:pb-[14px]">
      <h1 className="min-w-0 flex-[1_1_360px] text-[20px] leading-[1.2] font-semibold tracking-[-0.01em] text-dash-ink">
        {t("title")}
      </h1>
      <HeaderControls
        period={period}
        periods={OVERVIEW_PERIODS}
        scope="all"
        pathname="/overview"
        keep={{}}
      />
    </header>
  );
}
