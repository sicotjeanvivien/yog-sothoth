/** The Overview's bar series: fixed slots ending at `asOf`, and the scale's two round marks. */

import { HOUR_MS, periodMs, type Period } from "./period";

/** The tallest bar, under the chart's 170 px, so the top mark's label stays clear. */
export const CHART_TOP_PX = 160;

/** One slot of the series; `value` is `null` where the facts hold no whole bar. */
export type Slot = { from: Date; to: Date; value: number | null };

type Bar = { from: string; volumeUsd: string | null };

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
    const value = bar.volumeUsd === null ? NaN : Number.parseFloat(bar.volumeUsd);
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
