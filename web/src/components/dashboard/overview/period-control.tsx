/**
 * The period selector of a page's header, each period open or closed as `/api/windows` says.
 * ⚠️ A failed read keeps them links.
 */

import { getTranslations } from "next-intl/server";

import type { PagePath } from "@/config/site";
import { ApiClientError } from "@/lib/api/errors";
import { fetchWindows } from "@/lib/api/server/protocols";
import type { Period } from "@/lib/overview/period";

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

export async function PeriodControl({ period, periods, pathname, keep }: ControlProps) {
  let windows;
  try {
    ({ items: windows } = await fetchWindows());
  } catch (err) {
    if (!(err instanceof ApiClientError)) {
      throw err;
    }
    windows = null;
  }
  const entries = await Promise.all(
    periods.map(async (p) => {
      const row = windows?.find((w) => w.window === p);
      const reason = row === undefined ? null : await closedReason(row);
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
