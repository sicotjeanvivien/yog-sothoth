/** When the Overview's figures are late: past the ~70 min a frozen hour normally lags. */

import { HOUR_MS } from "./period";

export const STALE_AFTER_MS = 2 * HOUR_MS;

/** Late when `asOf` is over two hours back; before the first freeze, nothing is late. */
export function isStale(asOf: string | null, now: Date): boolean {
  return asOf !== null && now.getTime() - Date.parse(asOf) > STALE_AFTER_MS;
}
