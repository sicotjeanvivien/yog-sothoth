/** When the Overview's figures are late: well past the lag of a frozen hour. */

import { HOUR_MS } from "./period";

export const STALE_AFTER_MS = 2 * HOUR_MS;

/** Late when `asOf` is over two hours back; before the first freeze, nothing is late. */
export function isStale(asOf: string | null, now: Date): boolean {
  return asOf !== null && now.getTime() - Date.parse(asOf) > STALE_AFTER_MS;
}
