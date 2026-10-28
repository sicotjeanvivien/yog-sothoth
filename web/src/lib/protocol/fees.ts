/** The fees tab's chart, as data: each bar's fees split between the LPs, the protocol and the referral. */

import { slotsOf } from "@/lib/overview/chart";
import type { Period } from "@/lib/overview/period";

/** Where a swap's fees go, in the order the bars stack them, bottom first. */
export const FEE_PARTS = ["lp", "protocol", "referral"] as const;

export type FeePart = (typeof FEE_PARTS)[number];

/** One bar: each part `null` where the bar holds no valued fee. */
export type FeeSlot = { from: Date; to: Date } & Record<FeePart, number | null>;

type FeeBar = {
  from: string;
  lpFeesUsd: string | null;
  protocolFeesUsd: string | null;
  referralFeesUsd: string | null;
};

/** The fees in the period's slots, each part summed as `slotsOf` sums a bar. */
export function feeSlotsOf(period: Period, asOf: string, bars: readonly FeeBar[]): FeeSlot[] {
  const slots = (pick: (bar: FeeBar) => string | null) =>
    slotsOf(
      period,
      asOf,
      bars.map((bar) => ({ from: bar.from, value: pick(bar) })),
    );
  const lp = slots((bar) => bar.lpFeesUsd);
  const protocol = slots((bar) => bar.protocolFeesUsd);
  const referral = slots((bar) => bar.referralFeesUsd);
  return lp.map((slot, i) => ({
    from: slot.from,
    to: slot.to,
    lp: slot.value,
    protocol: protocol[i]?.value ?? null,
    referral: referral[i]?.value ?? null,
  }));
}

/** The period's fees by part, a bar without a value adding nothing. */
export function feeTotalsOf(slots: readonly FeeSlot[]): Record<FeePart, number> {
  const totals = { lp: 0, protocol: 0, referral: 0 };
  for (const slot of slots) {
    for (const part of FEE_PARTS) {
      totals[part] += slot[part] ?? 0;
    }
  }
  return totals;
}

/** Each part's share of their sum; `null` without any fee, which has no split. */
export function feeSharesOf(parts: Record<FeePart, number>): Record<FeePart, number> | null {
  const total = parts.lp + parts.protocol + parts.referral;
  if (total <= 0) {
    return null;
  }
  return { lp: parts.lp / total, protocol: parts.protocol / total, referral: parts.referral / total };
}
