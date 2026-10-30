/** The Overview's bar series: fixed slots ending at `asOf`, and the scale's two round marks. */

import type { IntervalResponse } from "@/lib/api/schema/zod.gen";

import { HOUR_MS, type Period } from "./period";

/** Each chart's height, and its tallest bar a little under it so the top mark's label stays clear. */
export const CHART_PX = {
  volume: { height: 170, top: 160 },
  fees: { height: 80, top: 74 },
} as const;

/** One slot of the series; `value` is `null` where the facts hold no whole bar. */
export type Slot = { from: Date; to: Date; value: number | null };

type Bar = { from: string; value: string | null };

/** The grid a series says it serves, `null` bounds before any fact. */
export type SeriesGrid = {
  from: string | null;
  asOf: string | null;
  interval: IntervalResponse;
};

/** A series response, its rows the bars of its grid. */
export type Series<B> = SeriesGrid & { items: readonly B[] };

const INTERVAL_MS: Record<IntervalResponse, number> = {
  "5m": HOUR_MS / 12,
  "1h": HOUR_MS,
  "6h": 6 * HOUR_MS,
  "1d": 24 * HOUR_MS,
};

/** The grid's slots, each holding its bar's value; `null` without a valued bar. */
export function slotsOf(grid: SeriesGrid, bars: readonly Bar[]): Slot[] {
  if (grid.from === null || grid.asOf === null) {
    return [];
  }
  const step = INTERVAL_MS[grid.interval];
  const start = Date.parse(grid.from);
  const count = (Date.parse(grid.asOf) - start) / step;

  const values = Array.from({ length: count }, (): number | null => null);
  for (const bar of bars) {
    const i = (Date.parse(bar.from) - start) / step;
    const value = bar.value === null ? NaN : Number.parseFloat(bar.value);
    if (i in values && Number.isFinite(value)) {
      values[i] = value;
    }
  }

  return values.map((value, i) => {
    const from = start + i * step;
    return { from: new Date(from), to: new Date(from + step), value };
  });
}

/** How many 24-hour slices before the last slot slot `i` falls in. */
export function daysAgoOf(slots: readonly { from: Date }[], i: number): number {
  const elapsed = slots[slots.length - 1]!.from.getTime() - slots[i]!.from.getTime();
  return Math.floor(elapsed / (24 * HOUR_MS));
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
const AXIS_EVERY: Record<Period, number> = { "1h": 3, "24h": 6, "7d": 4, "30d": 7 };

/** "14:05", the slot's start in UTC. */
export function hourOf(date: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(date.getUTCHours())}:${pad(date.getUTCMinutes())}`;
}

/** Whether a period's slots are named by their time of day rather than by their day. */
export function readsClock(period: Period): boolean {
  return period === "1h" || period === "24h";
}

/** The axis under a series: times of day up to 24 h, day labels beyond, the last one always shown. */
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
          !readsClock(period)
            ? dayLabel(daysAgoOf(slots, i))
            : i === last
              ? `${hourOf(slot.from)} UTC`
              : hourOf(slot.from),
        ]
      : [],
  );
}
