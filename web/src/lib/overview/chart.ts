/** The Overview's bar series: fixed slots ending at `asOf`, and the scale's two round marks. */

import { HOUR_MS, periodMs, readsHours, SLOT_HOURS, type Period } from "./period";

/** Each chart's height, and its tallest bar a little under it so the top mark's label stays clear. */
export const CHART_PX = {
  volume: { height: 170, top: 160 },
  fees: { height: 80, top: 74 },
} as const;

/** One slot of the series; `value` is `null` where the facts hold no whole bar. */
export type Slot = { from: Date; to: Date; value: number | null };

type Bar = { from: string; value: string | null };

/**
 * Hours over 24 h, 6-hour slices over 7 days, 24-hour slices beyond: each slot sums the valued bars
 * it holds, across protocols, as the API's days do; `null` without any, or, `strict`, with one unknown.
 */
export function slotsOf(
  period: Period,
  asOf: string,
  bars: readonly Bar[],
  { strict = false }: { strict?: boolean } = {},
): Slot[] {
  const step = SLOT_HOURS[period] * HOUR_MS;
  const count = periodMs(period) / step;
  const start = Date.parse(asOf) - count * step;
  const barStep = readsHours(period) ? HOUR_MS : step;

  const sums = Array.from({ length: count }, (): number | null => null);
  const unknown = Array.from({ length: count }, () => false);
  for (const bar of bars) {
    const at = Date.parse(bar.from);
    const i = Math.floor((at - start) / step);
    // ⚠️ A bar off the slots' grid (read at another `asOf`) belongs to none of them.
    if (i < 0 || i >= count || (at - start) % barStep !== 0) continue;
    const value = bar.value === null ? NaN : Number.parseFloat(bar.value);
    if (Number.isFinite(value)) {
      sums[i] = (sums[i] ?? 0) + value;
    } else {
      unknown[i] = true;
    }
  }

  return sums.map((sum, i) => {
    const from = start + i * step;
    return {
      from: new Date(from),
      to: new Date(from + step),
      value: strict && unknown[i] ? null : sum,
    };
  });
}

/** How many 24-hour slices before `asOf` slot `i` of `count` falls in. */
export function daysAgoOf(period: Period, count: number, i: number): number {
  return Math.floor(((count - 1 - i) * SLOT_HOURS[period]) / 24);
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
const AXIS_EVERY: Record<Period, number> = { "24h": 6, "7d": 4, "30d": 7 };

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
  // ⚠️ Over 7 days, one label per day's four slices: the others' rule names today twice.
  const labelled = (i: number) =>
    period === "7d"
      ? (last - i) % every === 0
      : i === last || (i % every === 0 && i <= last - every / 2);
  return slots.flatMap((slot, i) =>
    labelled(i)
      ? [
          period !== "24h"
            ? dayLabel(daysAgoOf(period, slots.length, i))
            : i === last
              ? `${hourOf(slot.from)} UTC`
              : hourOf(slot.from),
        ]
      : [],
  );
}
