/** The volume tab's chart, as data: each bar's volume split between the rotation runs and the rest. */

import { slotsOf } from "@/lib/overview/chart";
import type { Period } from "@/lib/overview/period";

/**
 * One bar: `total` is `null` where no volume is valued; `inRuns` and `outside` are `null` where
 * the runs do not split every hour of it.
 */
export type SplitSlot = {
  from: Date;
  to: Date;
  total: number | null;
  inRuns: number | null;
  outside: number | null;
};

type SplitBar = {
  from: string;
  volumeUsd: string | null;
  inRunsUsd: string | null;
  outsideRunsUsd: string | null;
};

/** The protocol's volume in the period's slots, ending at `asOf`, as its runs split it. */
export function splitSlotsOf(period: Period, asOf: string, bars: readonly SplitBar[]): SplitSlot[] {
  const pick = (value: (bar: SplitBar) => string | null, strict: boolean) =>
    slotsOf(
      period,
      asOf,
      bars.map((bar) => ({ from: bar.from, value: value(bar) })),
      { strict },
    );
  const totals = pick((bar) => bar.volumeUsd, false);
  const inRuns = pick((bar) => bar.inRunsUsd, true);
  const outside = pick((bar) => bar.outsideRunsUsd, true);
  return totals.map((slot, i) => ({
    from: slot.from,
    to: slot.to,
    total: slot.value,
    inRuns: inRuns[i]?.value ?? null,
    outside: outside[i]?.value ?? null,
  }));
}

/** The first bar the runs split, `null` before any is. */
export function splitSinceOf(bars: readonly SplitBar[]): string | null {
  return bars.find((bar) => bar.inRunsUsd !== null)?.from ?? null;
}

/** The legend: the split bars summed, the share in runs `null` without volume, and what is unsplit. */
export function splitTotalsOf(slots: readonly SplitSlot[]): {
  inRuns: number;
  outside: number;
  share: number | null;
  unsplit: number;
} {
  let inRuns = 0;
  let outside = 0;
  let unsplit = 0;
  for (const slot of slots) {
    if (slot.inRuns !== null && slot.outside !== null) {
      inRuns += slot.inRuns;
      outside += slot.outside;
    } else {
      unsplit += slot.total ?? 0;
    }
  }
  return { inRuns, outside, share: inRuns + outside > 0 ? inRuns / (inRuns + outside) : null, unsplit };
}

/** "18 min" under an hour, "2 h 05" beyond, from whole minutes. */
export function durationOf(seconds: number): string {
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) {
    return `${minutes} min`;
  }
  return `${Math.floor(minutes / 60)} h ${String(minutes % 60).padStart(2, "0")}`;
}
