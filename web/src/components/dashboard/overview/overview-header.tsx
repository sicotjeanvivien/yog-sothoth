/**
 * The compact header: the title alone; at the right, "Live" over the period and the mode.
 * ⚠️ The period selector streams in its own unkeyed `<Suspense>`: a period change keeps it.
 */

import { Suspense } from "react";
import { getTranslations } from "next-intl/server";

import { LiveIndicator } from "@/components/dashboard/live-status/live-indicator";
import { ApiClientError } from "@/lib/api/errors";
import { fetchProtocolActivity } from "@/lib/api/server/protocols";
import { CLOSED_PERIODS, PERIODS, type Period } from "@/lib/overview/period";

import { AdvancedSwitch } from "./advanced-switch";
import { closedReason } from "./overview-parts";
import { PeriodSelector, type PeriodAvailability } from "./period-selector";
import { OverviewLiveRows, OverviewLiveRowsFallback } from "./overview-live-rows";

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
                <OverviewLiveRows period={period} />
              </Suspense>
            }
          />
        </span>
        <div className="flex flex-wrap items-center gap-3 rail:justify-end">
          <Suspense fallback={<Selector period={period} availability={null} />}>
            <PeriodControl period={period} />
          </Suspense>
          <AdvancedSwitch label={t("advanced")} />
        </div>
      </div>
    </header>
  );
}

/**
 * Each period, open or closed, read on the span any measure carries. ⚠️ A failed read keeps
 * them links: each block then says for itself what its period lacks.
 */
async function PeriodControl({ period }: { period: Period }) {
  const t = await getTranslations("Dashboard.Overview.period");

  let span;
  try {
    span = await fetchProtocolActivity(period);
  } catch (err) {
    if (!(err instanceof ApiClientError)) {
      throw err;
    }
    const availability = Object.fromEntries(
      PERIODS.map((p) => [
        p,
        CLOSED_PERIODS.has(p) ? { open: false, reason: t("closedSoon") } : { open: true },
      ]),
    ) as PeriodAvailability;
    return <Selector period={period} availability={availability} />;
  }

  const entries = await Promise.all(
    PERIODS.map(async (p) => {
      const reason = await closedReason(p, span);
      return [p, reason === null ? { open: true } : { open: false, reason }] as const;
    }),
  );

  return (
    <Selector
      period={period}
      availability={Object.fromEntries(entries) as PeriodAvailability}
    />
  );
}

async function Selector({
  period,
  availability,
}: {
  period: Period;
  availability: PeriodAvailability | null;
}) {
  const t = await getTranslations("Dashboard.Overview.period");
  const labels = Object.fromEntries(PERIODS.map((p) => [p, t(`button.${p}`)])) as Record<
    Period,
    string
  >;
  const closedLabels = Object.fromEntries(
    PERIODS.map((p) => [p, t("closedLabel", { period: labels[p] })]),
  ) as Record<Period, string>;

  return (
    <PeriodSelector
      current={period}
      availability={availability}
      labels={labels}
      closedLabels={closedLabels}
      groupLabel={t("label")}
    />
  );
}
