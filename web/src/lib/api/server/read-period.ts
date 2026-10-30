/** The period a page reads, chosen by `?period=` and by what the API opens. */

import { ApiClientError } from "@/lib/api/errors";
import {
  DEFAULT_PERIOD,
  parsePeriod,
  periodHeldClosed,
  type Period,
} from "@/lib/overview/period";

import { fetchWindows } from "./protocols";

/** 24h when a period is held closed, or when the windows cannot be read: its reads never run. */
export async function readPeriod<P extends Period>(
  raw: string | string[] | undefined,
  periods: readonly P[],
): Promise<P | typeof DEFAULT_PERIOD> {
  const period = parsePeriod(raw, periods);
  if (period === DEFAULT_PERIOD) {
    return period;
  }
  try {
    return periodHeldClosed(period, (await fetchWindows()).items) ? DEFAULT_PERIOD : period;
  } catch (err) {
    if (err instanceof ApiClientError) {
      return DEFAULT_PERIOD;
    }
    throw err;
  }
}
