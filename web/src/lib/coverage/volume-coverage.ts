/**
 * Coverage of a pool's 24h volume — how many of the hours that actually traded
 * could be valued in USD.
 *
 * `SUM` skips the hours it cannot value, so a bare `volume24hUsd` can be a
 * sub-total wearing a total's clothes: 117 787 $ over 7 of the 8 hours that
 * traded reads exactly like 117 787 $ over all 8. The API already ships the two
 * counters that tell them apart (`swapBuckets24h` / `swapBucketsPriced24h`);
 * this is the one place that decides what they mean for a reader.
 *
 * Pure and shared so the rule lives once, for the four surfaces that state it:
 * the pool-detail KPIs, the `/pools` table (and the watchlist that reuses its
 * row), the Overview ranking, and the Overview volume KPI — that last one over
 * the GLOBAL counters, pool-hours summed across every pool rather than one
 * pool's own. The shape is the same, which is why the parameter is structural
 * rather than a `PoolResponse`.
 *
 * ⚠️ **No threshold, deliberately.** The mark fires on "not fully covered", a
 * fact — not on "below x %", a number nobody has chosen. Penalising thin
 * coverage is a separate decision about the ORDER (`.project` ticket 03,
 * options A and B), and it needs a measurement we do not have yet.
 */

/** What the reader is told about one pool's 24h volume figure. */
export type VolumeCoverage = {
  /** Hours of the window that traded AND could be valued. */
  priced: number;
  /** Hours of the window that traded at all — the denominator. */
  total: number;
  /** `true` when the published figure is a sub-total, not a total. */
  partial: boolean;
};

/**
 * `null` when the pool did not trade in the window: there is no figure to
 * qualify, and a "0 / 0" would read as a failure rather than as silence.
 */
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
