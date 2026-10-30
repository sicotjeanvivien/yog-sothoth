/** Sums and changes over the API's decimal strings, for display only. */

import { periodMs, type Period } from "./period";

/** A decimal string of the API as a number, `null` where it is unknown. */
export function amountOf(value: string | null | undefined): number | null {
  const parsed = value === null || value === undefined ? NaN : Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : null;
}

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

/** `part / whole`, or `null` when either is unknown or the whole is zero. */
export function shareOf(part: number | null, whole: number | null): number | null {
  if (part === null || whole === null || whole <= 0) {
    return null;
  }
  return part / whole;
}

/** The average number of swaps per second over the whole period. */
export function swapsPerSecond(swaps: number, period: Period): number {
  return swaps / (periodMs(period) / 1000);
}
