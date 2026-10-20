/**
 * The coverage of a volume: how many of the pool-hours that traded could be valued, since
 * `SUM` skips the rest. The one rule for every surface. ⚠️ No threshold: the mark fires on
 * "not fully covered", a fact, not on a percentage nobody has chosen.
 */

export type VolumeCoverage = {
  /** Pool-hours of the window that traded AND could be valued. */
  priced: number;
  /** Pool-hours of the window that traded at all. */
  total: number;
  /** `true` when the published figure is a sub-total, not a total. */
  partial: boolean;
};

/** `null` when nothing traded: "0 / 0" would read as a failure, not as silence. */
export function volumeCoverage(counters: {
  poolHours: number;
  poolHoursValued: number;
}): VolumeCoverage | null {
  const total = counters.poolHours;
  if (total <= 0) {
    return null;
  }
  const priced = counters.poolHoursValued;
  return { priced, total, partial: priced < total };
}
