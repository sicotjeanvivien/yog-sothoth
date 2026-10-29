/**
 * The period selector of a page's header, each period open or closed on the span its family
 * carries. ⚠️ A failed read keeps them links: each block then says what its period lacks.
 */

import { getTranslations } from "next-intl/server";

import type { PagePath } from "@/config/site";
import { ApiClientError } from "@/lib/api/errors";
import { fetchSwapTotals } from "@/lib/api/server/protocols";
import type { Scope } from "@/lib/api/server/scope";
import { CLOSED_PERIODS, DEFAULT_PERIOD, type Period } from "@/lib/overview/period";

import { closedReason } from "./overview-parts";
import { PeriodSelector, type PeriodAvailability } from "./period-selector";

type ControlProps = {
  period: Period;
  /** The periods the page offers, in their order. */
  periods: readonly Period[];
  pathname: PagePath;
  /** The page's other query parameters, kept across a period change. */
  keep: Record<string, string>;
};

export async function PeriodControl({
  period,
  periods,
  scope,
  pathname,
  keep,
}: ControlProps & { scope: Scope }) {
  const t = await getTranslations("Dashboard.Overview.period");

  // ⚠️ The last hour has its own span, from the tranches still held: 30d judged on it would close.
  const [frozen, hour] = await Promise.allSettled([
    fetchSwapTotals(scope, period === "1h" ? DEFAULT_PERIOD : period),
    periods.includes("1h") ? fetchSwapTotals(scope, "1h") : null,
  ]);
  const entries = await Promise.all(
    periods.map(async (p) => {
      const span = p === "1h" ? hour : frozen;
      if (span.status === "rejected") {
        if (!(span.reason instanceof ApiClientError)) {
          throw span.reason;
        }
        const open = !CLOSED_PERIODS.has(p);
        return [p, open ? { open } : { open, reason: t("closedSoon") }] as const;
      }
      const reason = span.value === null ? null : await closedReason(p, span.value);
      return [p, reason === null ? { open: true } : { open: false, reason }] as const;
    }),
  );

  return (
    <PeriodSelect
      {...{ period, periods, pathname, keep }}
      availability={Object.fromEntries(entries) as PeriodAvailability}
    />
  );
}

/** The selector itself; without `availability`, its waiting state. */
export async function PeriodSelect({
  period,
  periods,
  pathname,
  keep,
  availability,
}: ControlProps & { availability: PeriodAvailability | null }) {
  const t = await getTranslations("Dashboard.Overview.period");
  const labels = Object.fromEntries(periods.map((p) => [p, t(`button.${p}`)])) as Record<
    Period,
    string
  >;
  const closedLabels = Object.fromEntries(
    periods.map((p) => [p, t("closedLabel", { period: labels[p] })]),
  ) as Record<Period, string>;

  return (
    <PeriodSelector
      periods={periods}
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
