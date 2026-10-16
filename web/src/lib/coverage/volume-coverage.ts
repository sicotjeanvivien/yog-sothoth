/**
 * The coverage of a 24h volume: how many of the hours that traded could be valued, since
 * `SUM` skips the rest. The one rule for every surface, per pool or global. ⚠️ No threshold:
 * the mark fires on "not fully covered", a fact, not on a percentage nobody has chosen.
 */

export type VolumeCoverage = {
  /** Hours of the window that traded AND could be valued. */
  priced: number;
  /** Hours of the window that traded at all. */
  total: number;
  /** `true` when the published figure is a sub-total, not a total. */
  partial: boolean;
};

/** `null` when nothing traded: "0 / 0" would read as a failure, not as silence. */
export function volumeCoverage(counters: {
  swapBuckets24h: number;
  swapBucketsPriced24h: number;
}): VolumeCoverage | null {
  const total = counters.swapBuckets24h;
  if (total <= 0) {
    return null;
  }
  const priced = counters.swapBucketsPriced24h;
  return { priced, total, partial: priced < total };
}
