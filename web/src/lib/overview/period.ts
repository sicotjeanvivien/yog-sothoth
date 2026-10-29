/** A page's period: the `window` of every measure it reads, chosen by `?period=`. */

import { zWindowResponse, type WindowResponse } from "@/lib/api/schema/zod.gen";

export type Period = WindowResponse;

/** A period of frozen hours: every one but the last hour, read from the 5-minute tranches. */
export type FrozenPeriod = Exclude<Period, "1h">;

export const OVERVIEW_PERIODS: readonly Period[] = ["1h", "24h", "7d", "30d"];

export const PROTOCOL_PERIODS: readonly FrozenPeriod[] = ["24h", "7d", "30d"];

export const DEFAULT_PERIOD = "24h" satisfies FrozenPeriod;

export const HOUR_MS = 60 * 60 * 1000;

const MINUTE_MS = 60 * 1000;

export const PERIOD_HOURS: Record<Period, number> = {
  "1h": 1,
  "24h": 24,
  "7d": 7 * 24,
  "30d": 30 * 24,
};

/** A slot's width over each period. */
export const SLOT_MS: Record<Period, number> = {
  "1h": 5 * MINUTE_MS,
  "24h": HOUR_MS,
  "7d": 6 * HOUR_MS,
  "30d": 24 * HOUR_MS,
};

/** The width of a bar the API returns: 5-minute tranches, hours summed into slots, or days. */
export const BAR_MS: Record<Period, number> = {
  "1h": 5 * MINUTE_MS,
  "24h": HOUR_MS,
  "7d": HOUR_MS,
  "30d": 24 * HOUR_MS,
};

/** What a period's coverage counts: pools × its bars, tranches under an hour. */
export function coverageUnit(period: Period): "poolHours" | "poolTranches" {
  return BAR_MS[period] < HOUR_MS ? "poolTranches" : "poolHours";
}

/** The `interval` a period's series is read at, one per bar of `BAR_MS`. */
export const SERIES_INTERVAL: Record<Period, "5m" | "1h" | "1d"> = {
  "1h": "5m",
  "24h": "1h",
  "7d": "1h",
  "30d": "1d",
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
 * The default for a missing, repeated, unknown or closed value, or one the page does not offer:
 * a bad link still renders a page, and a closed period's reads never run.
 */
export function parsePeriod<P extends Period>(
  raw: string | string[] | undefined,
  periods: readonly P[],
): P | typeof DEFAULT_PERIOD {
  const parsed = zWindowResponse.safeParse(raw);
  return parsed.success &&
    (periods as readonly Period[]).includes(parsed.data) &&
    !CLOSED_PERIODS.has(parsed.data)
    ? (parsed.data as P)
    : DEFAULT_PERIOD;
}
