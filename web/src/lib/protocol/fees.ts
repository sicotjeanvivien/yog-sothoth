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

/** A reading of what the LPs took or were owed over a period, as the API dates it. */
type Reading = { asOf: string | null; amountUsd: string | null };

/**
 * The fees claimed as a share of those generated for the LPs; `null` unless both readings end at
 * the same `asOf`. Above 1 is possible: a claim also takes fees generated before the period.
 */
export function claimedShareOf(claimed: Reading, generated: Reading): number | null {
  if (claimed.asOf === null || claimed.asOf !== generated.asOf) {
    return null;
  }
  const part = claimed.amountUsd === null ? null : Number.parseFloat(claimed.amountUsd);
  const whole = generated.amountUsd === null ? null : Number.parseFloat(generated.amountUsd);
  return shareOf(part, whole);
}
