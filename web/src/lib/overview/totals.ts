/** Sums and changes over the API's decimal strings, for display only. */

import type { ProtocolResponse } from "@/lib/api/schema/zod.gen";

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

/** `part / whole`, or `null` when either is unknown or the whole is zero. */
export function shareOf(part: number | null, whole: number | null): number | null {
  if (part === null || whole === null || whole <= 0) {
    return null;
  }
  return part / whole;
}

type Reading = { protocol: ProtocolResponse; at: string };

/**
 * The protocols' TVL held by the pools discovered in the period, each part read with its
 * protocol's TVL; `null` when one has no part measured at its reading.
 */
export function discoveredTvl(
  tvl: readonly Reading[],
  discovered: readonly (Reading & { tvlUsd: string })[],
): number | null {
  const parts = tvl.map(
    (reading) =>
      discovered.find(
        (d) => d.protocol === reading.protocol && Date.parse(d.at) === Date.parse(reading.at),
      )?.tvlUsd ?? null,
  );
  return sumUsd(parts);
}

/**
 * The TVL's change over the period without its discovered part: a pool seen for the first time
 * brings its whole stock, which the raw change would take for a deposit.
 */
export function tvlChange(
  current: number | null,
  start: number | null,
  discovered: number | null,
): number | null {
  return changeRatio(
    current === null || discovered === null ? null : current - discovered,
    start,
  );
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
