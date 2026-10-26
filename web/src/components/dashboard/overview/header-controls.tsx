/**
 * The right of a page's compact header: "Live" over the period and the mode. ⚠️ The live rows
 * are keyed by the period; the period selector streams in its own unkeyed `<Suspense>`, so a
 * period change keeps it.
 */

import { Suspense } from "react";
import { getTranslations } from "next-intl/server";

import { LiveIndicator } from "@/components/dashboard/live-status/live-indicator";
import type { PagePath } from "@/config/site";
import type { Period } from "@/lib/overview/period";
import type { Scope } from "@/lib/api/server/scope";

import { AdvancedSwitch } from "./advanced-switch";
import { OverviewLiveRows, OverviewLiveRowsFallback } from "./overview-live-rows";
import { PeriodControl, PeriodSelect } from "./period-control";

export async function HeaderControls({
  period,
  scope,
  pathname,
  keep,
}: {
  period: Period;
  scope: Scope;
  pathname: PagePath;
  /** The page's other query parameters, kept across a period change. */
  keep: Record<string, string>;
}) {
  const t = await getTranslations("Dashboard.Overview.page");
  const page = { pathname, keep };

  return (
    <div className="flex flex-col items-start gap-2.5 rail:ml-auto rail:items-end">
      <span className="-ml-[10px] rail:-mr-[10px] rail:ml-0">
        <LiveIndicator
          pageRows={
            <Suspense key={period} fallback={<OverviewLiveRowsFallback />}>
              <OverviewLiveRows period={period} scope={scope} />
            </Suspense>
          }
        />
      </span>
      <div className="flex flex-wrap items-center gap-3 rail:justify-end">
        <Suspense fallback={<PeriodSelect period={period} {...page} availability={null} />}>
          <PeriodControl period={period} scope={scope} {...page} />
        </Suspense>
        <AdvancedSwitch label={t("advanced")} />
      </div>
    </div>
  );
}
