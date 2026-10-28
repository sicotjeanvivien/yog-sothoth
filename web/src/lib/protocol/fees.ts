/** The fees tab's chart, as data: each bar's fees split between the LPs, the protocol and the referral. */

import { slotsOf } from "@/lib/overview/chart";
import type { Period } from "@/lib/overview/period";
import { shareOf } from "@/lib/overview/totals";

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

/** A bar's three parts, `null` unless all three are known: an unknown part is not 0. */
export function feePartsOf(slot: FeeSlot): Record<FeePart, number> | null {
  const { lp, protocol, referral } = slot;
  return lp === null || protocol === null || referral === null ? null : { lp, protocol, referral };
}

/** The whole the three parts make. */
export function feeSumOf(parts: Record<FeePart, number>): number {
  return parts.lp + parts.protocol + parts.referral;
}

/** The period's fees by part over its known bars; `null` without any. */
export function feeTotalsOf(slots: readonly FeeSlot[]): Record<FeePart, number> | null {
  let totals: Record<FeePart, number> | null = null;
  for (const slot of slots) {
    const parts = feePartsOf(slot);
    if (parts === null) continue;
    totals ??= { lp: 0, protocol: 0, referral: 0 };
    for (const part of FEE_PARTS) {
      totals[part] += parts[part];
    }
  }
  return totals;
}

/** Each part's share of their sum, `null` where they sum to nothing. */
export function feeSharesOf(parts: Record<FeePart, number>): Record<FeePart, number | null> {
  const whole = feeSumOf(parts);
  return {
    lp: shareOf(parts.lp, whole),
    protocol: shareOf(parts.protocol, whole),
    referral: shareOf(parts.referral, whole),
  };
}
