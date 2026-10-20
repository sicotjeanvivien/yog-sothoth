/**
 * The page's price coverage, for the "Live" panel: the share of the period's pool-hours
 * that could be valued. ⚠️ Rounded down: 99.6 % must not read as complete.
 */

import { getLocale, getTranslations } from "next-intl/server";

import { ApiClientError } from "@/lib/api/errors";
import { fetchProtocolActivity } from "@/lib/api/server/protocols";
import type { Period } from "@/lib/overview/period";

export async function PriceCoverage({ period }: { period: Period }) {
  const t = await getTranslations("Dashboard.Overview.coverage");
  const locale = await getLocale();

  let activity;
  try {
    activity = await fetchProtocolActivity(period);
  } catch (err) {
    if (err instanceof ApiClientError) {
      return "—";
    }
    throw err;
  }

  const total = activity.items.reduce((sum, i) => sum + i.poolHours, 0);
  if (total === 0) {
    return "—";
  }
  const valued = activity.items.reduce((sum, i) => sum + i.poolHoursValued, 0);
  const percent = new Intl.NumberFormat(locale, {
    style: "percent",
    maximumFractionDigits: 0,
    roundingMode: "floor",
  }).format(valued / total);
  return t("value", { percent });
}
