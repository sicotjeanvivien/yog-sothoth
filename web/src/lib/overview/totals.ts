/** Sums and changes over the API's decimal strings, for display only. */

import { periodMs, type Period } from "./period";

/** `null` when there is nothing to sum, or one term is unknown: a sum without it would lie. */
export function sumUsd(values: readonly (string | null)[]): number | null {
  if (values.length === 0) {
    return null;
  }
  let total = 0;
  for (const value of values) {
    const parsed = value === null ? NaN : Number.parseFloat(value);
    if (!Number.isFinite(parsed)) {
      return null;
    }
    total += parsed;
  }
  return total;
}

/** `current / previous − 1`, or `null` when either is unknown or the base is zero. */
export function changeRatio(current: number | null, previous: number | null): number | null {
  if (current === null || previous === null || previous <= 0) {
    return null;
  }
  return current / previous - 1;
}

/** Volume over TVL, or `null` when either is unknown or the TVL is zero. */
export function rotation(volume: number | null, tvl: number | null): number | null {
  if (volume === null || tvl === null || tvl <= 0) {
    return null;
  }
  return volume / tvl;
}

/** The average number of swaps per second over the whole period. */
export function swapsPerSecond(swaps: number, period: Period): number {
  return swaps / (periodMs(period) / 1000);
}
