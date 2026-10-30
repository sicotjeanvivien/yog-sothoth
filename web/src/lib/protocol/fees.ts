/** The fees tab's chart, as data: each bar's fees split between the LPs, the protocol and the referral. */

import { slotsOf, type Series } from "@/lib/overview/chart";
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

/** The fees in the series' slots, part by part. */
export function feeSlotsOf(series: Series<FeeBar>): FeeSlot[] {
  const slots = (pick: (bar: FeeBar) => string | null) =>
    slotsOf(
      series,
      series.items.map((bar) => ({ from: bar.from, value: pick(bar) })),
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

/** The parts of a bar or a period, each `null` where it is not known. */
export type FeeParts = Record<FeePart, number | null>;

/** The sum of the known parts, as the API's own sums skip a value not known. */
export function feeSumOf(parts: FeeParts): number {
  return FEE_PARTS.reduce((sum, part) => sum + (parts[part] ?? 0), 0);
}

/** The period's fees by part, each `null` where no bar knows it; `null` without any part known. */
export function feeTotalsOf(slots: readonly FeeSlot[]): FeeParts | null {
  const totals: FeeParts = { lp: null, protocol: null, referral: null };
  for (const slot of slots) {
    for (const part of FEE_PARTS) {
      const value = slot[part];
      if (value !== null) {
        totals[part] = (totals[part] ?? 0) + value;
      }
    }
  }
  return FEE_PARTS.every((part) => totals[part] === null) ? null : totals;
}

/** Each part's share of the known parts' sum, `null` where it is not known or they sum to nothing. */
export function feeSharesOf(parts: FeeParts): FeeParts {
  const whole = feeSumOf(parts);
  return {
    lp: shareOf(parts.lp, whole),
    protocol: shareOf(parts.protocol, whole),
    referral: shareOf(parts.referral, whole),
  };
}
