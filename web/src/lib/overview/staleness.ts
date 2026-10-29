/** When the Overview's figures are late: well past the lag of a frozen hour, or of a tranche. */

import { HOUR_MS, type Period } from "./period";

export const STALE_AFTER_MS: Record<Period, number> = {
  "1h": HOUR_MS / 2,
  "24h": 2 * HOUR_MS,
  "7d": 2 * HOUR_MS,
  "30d": 2 * HOUR_MS,
};

/** Late when `asOf` is past the period's threshold; before the first freeze, nothing is late. */
export function isStale(asOf: string | null, now: Date, period: Period): boolean {
  return asOf !== null && now.getTime() - Date.parse(asOf) > STALE_AFTER_MS[period];
}
