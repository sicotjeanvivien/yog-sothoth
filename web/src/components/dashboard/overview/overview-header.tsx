/**
 * The compact header: breadcrumb and title on one line; at the right, "Live" over the period.
 * ⚠️ The period selector streams in its own unkeyed `<Suspense>`: a period change keeps it.
 */

import { Suspense } from "react";
import { getTranslations } from "next-intl/server";

import { LiveIndicator } from "@/components/dashboard/live-status/live-indicator";
import { ApiClientError } from "@/lib/api/errors";
import { fetchProtocolActivity } from "@/lib/api/server/protocols";
import { periodState } from "@/lib/overview/availability";
import { PERIODS, type Period } from "@/lib/overview/period";

import { LABEL_CLASS } from "./overview-layout";
import { utcMoment } from "./overview-parts";
import { PeriodSelector, type PeriodAvailability } from "./period-selector";
import { PriceCoverage } from "./price-coverage";

export async function OverviewHeader({ period }: { period: Period }) {
  const t = await getTranslations("Dashboard.Overview.page");

  return (
    <header className="flex flex-wrap items-start justify-between gap-x-8 gap-y-3 border-b border-dash-rule px-4 py-5 font-dash-sans rail:px-8 rail:pt-4 rail:pb-[14px]">
      <div className="flex min-w-0 flex-[1_1_360px] flex-wrap items-baseline gap-x-2 gap-y-1.5">
        <span className={LABEL_CLASS}>{t("breadcrumb")}</span>
        <h1 className="text-[20px] leading-[1.2] font-semibold tracking-[-0.01em] text-dash-ink">
          <span aria-hidden="true" className="font-normal text-dash-ink-3">
            /{" "}
          </span>
          {t("title")}
        </h1>
      </div>
      <div className="flex flex-col items-start gap-2.5 rail:ml-auto rail:items-end">
        <span className="-ml-[10px] rail:-mr-[10px] rail:ml-0">
          <LiveIndicator
            coverage={
              <Suspense key={period} fallback="—">
                <PriceCoverage period={period} />
              </Suspense>
            }
          />
        </span>
        <Suspense fallback={<Selector period={period} availability={null} />}>
          <PeriodControl period={period} />
        </Suspense>
      </div>
    </header>
  );
}

/** Each period, open or closed, read on the span any measure carries. */
async function PeriodControl({ period }: { period: Period }) {
  const t = await getTranslations("Dashboard.Overview.period");

  let span;
  try {
    span = await fetchProtocolActivity(period);
  } catch (err) {
    if (err instanceof ApiClientError) {
      return <Selector period={period} availability={null} />;
    }
    throw err;
  }

  const since = span.coveredSince ? await utcMoment(span.coveredSince) : null;
  const entries = await Promise.all(
    PERIODS.map(async (p) => {
      const state = periodState(p, span);
      if (state.open) {
        return [p, state] as const;
      }
      const reason =
        state.opensAt !== null && since !== null
          ? t("closedUntil", { since, opens: await utcMoment(state.opensAt) })
          : span.asOf === null
            ? t("closedEmpty")
            : t("closedSoon");
      return [p, { open: false, reason }] as const;
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
