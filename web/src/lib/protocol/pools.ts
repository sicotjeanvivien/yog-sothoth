/** The pools tab's chart, as data: the pools born in each bar. */

import { slotsOf, type Slot } from "@/lib/overview/chart";
import type { Period } from "@/lib/overview/period";

type BirthBar = { from: string; pools: number };

/** The pools born in the period's slots; a slot without any bar is `null`, not 0. */
export function birthSlotsOf(period: Period, asOf: string, bars: readonly BirthBar[]): Slot[] {
  return slotsOf(
    period,
    asOf,
    bars.map((bar) => ({ from: bar.from, value: String(bar.pools) })),
  );
}

/** The period's births: the slots summed, `null` when none holds a bar. */
export function birthTotalOf(slots: readonly Slot[]): number | null {
  return slots.reduce<number | null>(
    (total, slot) => (slot.value === null ? total : (total ?? 0) + slot.value),
    null,
  );
}
