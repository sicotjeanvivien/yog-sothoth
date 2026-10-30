import type { IntervalResponse } from "@/lib/api/schema/zod.gen";

import type { Series } from "../chart";

const SPAN_HOURS: Record<IntervalResponse, number> = { "5m": 1, "1h": 24, "6h": 168, "1d": 720 };

/** A series as the API serves it: the window its `interval` is the default of, ending at `asOf`. */
export function seriesOf<B>(interval: IntervalResponse, asOf: string, items: readonly B[]): Series<B> {
  const from = new Date(Date.parse(asOf) - SPAN_HOURS[interval] * 3_600_000).toISOString();
  return { from, asOf, interval, items };
}
