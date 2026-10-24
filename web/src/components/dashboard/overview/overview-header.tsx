/**
 * The compact header: the title alone; at the right, "Live" over the period and the mode.
 * ⚠️ The period selector streams in its own unkeyed `<Suspense>`: a period change keeps it.
 */

import { Suspense } from "react";
import { getTranslations } from "next-intl/server";

import { LiveIndicator } from "@/components/dashboard/live-status/live-indicator";
import type { Period } from "@/lib/overview/period";

import { AdvancedSwitch } from "./advanced-switch";
import { OverviewLiveRows, OverviewLiveRowsFallback } from "./overview-live-rows";
import { PeriodControl, PeriodSelect } from "./period-control";

const PAGE = { pathname: "/overview", keep: {} } as const;

export async function OverviewHeader({ period }: { period: Period }) {
  const t = await getTranslations("Dashboard.Overview.page");

  return (
    <header className="flex flex-wrap items-start justify-between gap-x-8 gap-y-3 border-b border-dash-rule px-4 py-5 font-dash-sans rail:px-8 rail:pt-4 rail:pb-[14px]">
      <h1 className="min-w-0 flex-[1_1_360px] text-[20px] leading-[1.2] font-semibold tracking-[-0.01em] text-dash-ink">
        {t("title")}
      </h1>
      <div className="flex flex-col items-start gap-2.5 rail:ml-auto rail:items-end">
        <span className="-ml-[10px] rail:-mr-[10px] rail:ml-0">
          <LiveIndicator
            pageRows={
              <Suspense key={period} fallback={<OverviewLiveRowsFallback />}>
                <OverviewLiveRows period={period} scope="all" />
              </Suspense>
            }
          />
        </span>
        <div className="flex flex-wrap items-center gap-3 rail:justify-end">
          <Suspense fallback={<PeriodSelect period={period} {...PAGE} availability={null} />}>
            <PeriodControl period={period} {...PAGE} />
          </Suspense>
          <AdvancedSwitch label={t("advanced")} />
        </div>
      </div>
    </header>
  );
}
