/** The Overview's period: the `window` of every measure it reads, chosen by `?period=`. */

import { zWindowResponse, type WindowResponse } from "@/lib/api/schema/zod.gen";

export type Period = WindowResponse;

export const PERIODS: readonly Period[] = zWindowResponse.options;

export const DEFAULT_PERIOD: Period = "24h";

export const HOUR_MS = 60 * 60 * 1000;

export const PERIOD_HOURS: Record<Period, number> = {
  "24h": 24,
  "7d": 7 * 24,
  "30d": 30 * 24,
};

/** Closed whatever the facts hold, until its own times are measured on the prod. */
export const CLOSED_PERIODS: ReadonlySet<Period> = new Set<Period>(["30d"]);

/** A page's query: `period` left out at its default, the page's other parameters kept. */
export function pageQuery(
  period: Period,
  keep: Record<string, string>,
): Record<string, string> {
  return period === DEFAULT_PERIOD ? keep : { ...keep, period };
}

export function periodMs(period: Period): number {
  return PERIOD_HOURS[period] * HOUR_MS;
}

/**
 * The default for a missing, repeated, unknown or closed value: a bad link still renders a
 * page, and a closed period's reads never run.
 */
export function parsePeriod(raw: string | string[] | undefined): Period {
  const parsed = zWindowResponse.safeParse(raw);
  return parsed.success && !CLOSED_PERIODS.has(parsed.data) ? parsed.data : DEFAULT_PERIOD;
}
