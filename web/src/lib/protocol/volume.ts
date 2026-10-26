/** The volume tab's chart, as data: each bar's volume split between the rotation runs and the rest. */

import { slotsOf } from "@/lib/overview/chart";
import type { Period } from "@/lib/overview/period";

/**
 * One bar: `total` is `null` where no volume is valued; `inRuns` is `null` where the runs do not
 * cover the bar whole, so it cannot be split.
 */
export type SplitSlot = {
  from: Date;
  to: Date;
  total: number | null;
  inRuns: number | null;
};

type VolumeBar = { from: string; volumeUsd: string | null };

/** The protocol's volume in the period's slots, ending at the swaps' `asOf`, split by its runs. */
export function splitSlotsOf(
  period: Period,
  asOf: string,
  swaps: readonly VolumeBar[],
  runs: { asOf: string | null; coveredSince: string | null; items: readonly VolumeBar[] },
): SplitSlot[] {
  const pick = (bars: readonly VolumeBar[]) =>
    slotsOf(
      period,
      asOf,
      bars.map((bar) => ({ from: bar.from, value: bar.volumeUsd })),
    );
  const totals = pick(swaps);
  const inRuns = pick(runs.items);
  const coveredFrom = runs.coveredSince === null ? Infinity : Date.parse(runs.coveredSince);
  // ⚠️ Read at another freeze, the runs' days start at other hours: no bar of theirs is ours.
  const sameEnd = runs.asOf !== null && Date.parse(runs.asOf) === Date.parse(asOf);
  return totals.map((slot, i) => {
    const covered = sameEnd && slot.from.getTime() >= coveredFrom;
    return {
      from: slot.from,
      to: slot.to,
      total: slot.value,
      inRuns: slot.value === null || !covered ? null : (inRuns[i]?.value ?? 0),
    };
  });
}

/** What a split bar shows: the volume outside the runs, never under zero. */
export function outsideRuns(slot: SplitSlot): number | null {
  return slot.total === null || slot.inRuns === null ? null : Math.max(0, slot.total - slot.inRuns);
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
    const out = outsideRuns(slot);
    if (slot.inRuns !== null && out !== null) {
      inRuns += slot.inRuns;
      outside += out;
    } else {
      unsplit += slot.total ?? 0;
    }
  }
  return { inRuns, outside, share: inRuns + outside > 0 ? inRuns / (inRuns + outside) : null, unsplit };
}

/**
 * Since when the runs are measured, `null` before any is. ⚠️ Not `coveredSince` alone: the API
 * sets it to `asOf` while no run is measured.
 */
export function runsMeasuredSince(runs: { asOf: string | null; coveredSince: string | null }): string | null {
  return runs.coveredSince !== null &&
    runs.asOf !== null &&
    Date.parse(runs.coveredSince) < Date.parse(runs.asOf)
    ? runs.coveredSince
    : null;
}

/** "18 min" under an hour, "2 h 05" beyond, from whole minutes. */
export function durationOf(seconds: number): string {
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) {
    return `${minutes} min`;
  }
  return `${Math.floor(minutes / 60)} h ${String(minutes % 60).padStart(2, "0")}`;
}
