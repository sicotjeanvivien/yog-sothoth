/**
 * The rows of the "Live" panel: the share of the period's coverage of `scope` that could be
 * valued, and the hour every figure stops at. ⚠️ Rounded down: 99.6 % is not complete.
 */

import { getLocale, getTranslations } from "next-intl/server";

import { LiveRow } from "@/components/dashboard/live-status/live-indicator";
import { ApiClientError } from "@/lib/api/errors";
import { fetchSwapTotals } from "@/lib/api/server/protocols";
import type { Period } from "@/lib/overview/period";
import type { Scope } from "@/lib/api/server/scope";

import { utcMoment } from "./overview-parts";

export async function OverviewLiveRows({ period, scope }: { period: Period; scope: Scope }) {
  const t = await getTranslations("Dashboard");
  const locale = await getLocale();

  let activity;
  try {
    activity = await fetchSwapTotals(scope, period);
  } catch (err) {
    if (!(err instanceof ApiClientError)) {
      throw err;
    }
    return <OverviewLiveRowsFallback />;
  }

  const total = activity.total?.poolHours ?? 0;
  const valued = activity.total?.poolHoursValued ?? 0;
  const percent = new Intl.NumberFormat(locale, {
    style: "percent",
    maximumFractionDigits: 0,
    roundingMode: "floor",
  }).format(total === 0 ? 0 : valued / total);

  return (
    <>
      <LiveRow
        label={t("liveStatus.priceCoverage")}
        value={total === 0 ? "—" : t(`Overview.coverage.${activity.coverageUnit}.value`, { percent })}
      />
      <LiveRow
        label={t("liveStatus.measuredUntil")}
        value={activity.asOf === null ? "—" : await utcMoment(activity.asOf)}
        late={activity.late}
      />
    </>
  );
}

export async function OverviewLiveRowsFallback() {
  const t = await getTranslations("Dashboard.liveStatus");
  return (
    <>
      <LiveRow label={t("priceCoverage")} value="—" />
      <LiveRow label={t("measuredUntil")} value="—" />
    </>
  );
}
