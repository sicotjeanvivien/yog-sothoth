/** The Overview's bar series: fixed slots ending at `asOf`, and the scale's two round marks. */

import { HOUR_MS, periodMs, type Period } from "./period";

/** Each chart's height, and its tallest bar a little under it so the top mark's label stays clear. */
export const CHART_PX = {
  volume: { height: 170, top: 160 },
  fees: { height: 80, top: 74 },
} as const;

/** One slot of the series; `value` is `null` where the facts hold no whole bar. */
export type Slot = { from: Date; to: Date; value: number | null };

type Bar = { from: string; value: string | null };

/** Hours over 24 h, 24-hour slices beyond: one slot each, summed across protocols. */
export function slotsOf(
  period: Period,
  asOf: string,
  bars: readonly Bar[],
): Slot[] {
  const step = period === "24h" ? HOUR_MS : 24 * HOUR_MS;
  const end = Date.parse(asOf);
  const count = periodMs(period) / step;

  const sums = new Map<number, number>();
  for (const bar of bars) {
    const value = bar.value === null ? NaN : Number.parseFloat(bar.value);
    if (!Number.isFinite(value)) continue;
    const key = Date.parse(bar.from);
    sums.set(key, (sums.get(key) ?? 0) + value);
  }

  return Array.from({ length: count }, (_, i) => {
    const from = end - (count - i) * step;
    return {
      from: new Date(from),
      to: new Date(from + step),
      value: sums.get(from) ?? null,
    };
  });
}

/**
 * The two dashed marks: the largest d × 10ⁿ under `max`, or the largest half step when that
 * falls under 60 % of it, and its half.
 */
export function roundTicks(max: number): number[] {
  if (!Number.isFinite(max) || max <= 0) {
    return [];
  }
  const power = 10 ** Math.floor(Math.log10(max));
  let top = Math.floor(max / power) * power;
  if (top / max < 0.6) {
    top = Math.floor(max / (power / 2)) * (power / 2);
  }
  return [top, top / 2];
}

/** The step between axis labels; the last slot always has one. */
const AXIS_EVERY: Record<Period, number> = { "24h": 6, "7d": 1, "30d": 7 };

/** "14:00", the slot's hour in UTC. */
export function hourOf(date: Date): string {
  return `${String(date.getUTCHours()).padStart(2, "0")}:00`;
}

/** The axis under a series: hours over 24 h, day labels beyond, the last one always shown. */
export function axisOf(
  period: Period,
  slots: readonly { from: Date }[],
  dayLabel: (daysAgo: number) => string,
): string[] {
  const every = AXIS_EVERY[period];
  const last = slots.length - 1;
  return slots.flatMap((slot, i) =>
    i === last || (i % every === 0 && i <= last - every / 2)
      ? [
          period !== "24h"
            ? dayLabel(last - i)
            : i === last
              ? `${hourOf(slot.from)} UTC`
              : hourOf(slot.from),
        ]
      : [],
  );
}
