/**
 * The period selector of a page's header, each period open or closed on the span any measure
 * carries. ⚠️ A failed read keeps them links: each block then says what its period lacks.
 */

import { getTranslations } from "next-intl/server";

import type { PagePath } from "@/config/site";
import { ApiClientError } from "@/lib/api/errors";
import { fetchSwapTotals } from "@/lib/api/server/protocols";
import type { Scope } from "@/lib/api/server/scope";
import { CLOSED_PERIODS, PERIODS, type Period } from "@/lib/overview/period";

import { closedReason } from "./overview-parts";
import { PeriodSelector, type PeriodAvailability } from "./period-selector";

type ControlProps = {
  period: Period;
  pathname: PagePath;
  /** The page's other query parameters, kept across a period change. */
  keep: Record<string, string>;
};

export async function PeriodControl({
  period,
  scope,
  pathname,
  keep,
}: ControlProps & { scope: Scope }) {
  const t = await getTranslations("Dashboard.Overview.period");

  let span;
  try {
    span = await fetchSwapTotals(scope, period);
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
    return <PeriodSelect {...{ period, pathname, keep }} availability={availability} />;
  }

  const entries = await Promise.all(
    PERIODS.map(async (p) => {
      const reason = await closedReason(p, span);
      return [p, reason === null ? { open: true } : { open: false, reason }] as const;
    }),
  );

  return (
    <PeriodSelect
      {...{ period, pathname, keep }}
      availability={Object.fromEntries(entries) as PeriodAvailability}
    />
  );
}

/** The selector itself; without `availability`, its waiting state. */
export async function PeriodSelect({
  period,
  pathname,
  keep,
  availability,
}: ControlProps & { availability: PeriodAvailability | null }) {
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
      pathname={pathname}
      keep={keep}
      current={period}
      availability={availability}
      labels={labels}
      closedLabels={closedLabels}
      groupLabel={t("label")}
    />
  );
}
