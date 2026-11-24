/** The fees tab's chart, as data: each bar's fees split between the LPs, the protocol and the referral. */

import { slotsOf, type Series } from "@/lib/overview/chart";
import { shareOf } from "@/lib/overview/totals";

/** Every part a bar or a period may show: the LPs' whole, or their fees claimed and compounded. */
export const FEE_PARTS = ["lp", "claiming", "compounding", "protocol", "referral"] as const;

export type FeePart = (typeof FEE_PARTS)[number];

/** Where a swap's fees go, in the order the bars stack them, bottom first, the LPs' split. */
const SPLIT_PARTS: readonly FeePart[] = ["claiming", "compounding", "protocol", "referral"];

/** The same, the LPs' fees whole: their split not measured. */
const WHOLE_PARTS: readonly FeePart[] = ["lp", "protocol", "referral"];

/** The parts of a bar or a period, each `null` where it is not known. */
export type FeeParts = Record<FeePart, number | null>;

/** One bar: each part `null` where the bar holds no valued fee. */
export type FeeSlot = { from: Date; to: Date } & FeeParts;

type FeeBar = {
  from: string;
  lpFeesUsd: string | null;
  protocolFeesUsd: string | null;
  referralFeesUsd: string | null;
  meteoraDammV2?: { claimingFeesUsd: string | null; compoundingFeesUsd: string | null } | undefined;
};

/** The fees in the series' slots, part by part. */
export function feeSlotsOf(series: Series<FeeBar>): FeeSlot[] {
  const slots = (pick: (bar: FeeBar) => string | null | undefined) =>
    slotsOf(
      series,
      series.items.map((bar) => ({ from: bar.from, value: pick(bar) ?? null })),
    );
  const lp = slots((bar) => bar.lpFeesUsd);
  const claiming = slots((bar) => bar.meteoraDammV2?.claimingFeesUsd);
  const compounding = slots((bar) => bar.meteoraDammV2?.compoundingFeesUsd);
  const protocol = slots((bar) => bar.protocolFeesUsd);
  const referral = slots((bar) => bar.referralFeesUsd);
  return lp.map((slot, i) => ({
    from: slot.from,
    to: slot.to,
    lp: slot.value,
    claiming: claiming[i]?.value ?? null,
    compounding: compounding[i]?.value ?? null,
    protocol: protocol[i]?.value ?? null,
    referral: referral[i]?.value ?? null,
  }));
}

/** The parts to show, in stacking order: the LPs' split where both its halves are known. */
export function feePartsOf(parts: FeeParts | null): readonly FeePart[] {
  return parts !== null && parts.claiming !== null && parts.compounding !== null
    ? SPLIT_PARTS
    : WHOLE_PARTS;
}

/** The sum of the known parts shown, as the API's own sums skip a value not known. */
export function feeSumOf(parts: FeeParts): number {
  return feePartsOf(parts).reduce((sum, part) => sum + (parts[part] ?? 0), 0);
}

/**
 * The period's fees by part, each `null` where no bar knows it; `null` without any part known.
 * ⚠️ The LPs' split only if every bar with LP fees has it: summed alone, it would miss theirs.
 */
export function feeTotalsOf(slots: readonly FeeSlot[]): FeeParts | null {
  const totals: FeeParts = { lp: null, claiming: null, compounding: null, protocol: null, referral: null };
  for (const slot of slots) {
    for (const part of FEE_PARTS) {
      const value = slot[part];
      if (value !== null) {
        totals[part] = (totals[part] ?? 0) + value;
      }
    }
  }
  if (slots.some((slot) => slot.lp !== null && (slot.claiming === null || slot.compounding === null))) {
    totals.claiming = null;
    totals.compounding = null;
  }
  return FEE_PARTS.every((part) => totals[part] === null) ? null : totals;
}

/** Each part's share of the known parts' sum, `null` where it is not known or they sum to nothing. */
export function feeSharesOf(parts: FeeParts): FeeParts {
  const whole = feeSumOf(parts);
  const shown = feePartsOf(parts);
  const share = (part: FeePart) => (shown.includes(part) ? shareOf(parts[part], whole) : null);
  return {
    lp: share("lp"),
    claiming: share("claiming"),
    compounding: share("compounding"),
    protocol: share("protocol"),
    referral: share("referral"),
  };
}
