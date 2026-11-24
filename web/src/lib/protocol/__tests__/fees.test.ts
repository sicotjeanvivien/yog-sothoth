import { describe, expect, it } from "vitest";

import { seriesOf } from "@/lib/overview/__tests__/series";
import { feePartsOf, feeSharesOf, feeSlotsOf, feeSumOf, feeTotalsOf } from "../fees";

const AS_OF = "2026-10-25T03:00:00Z";

const hour = (h: number) => new Date(Date.parse(AS_OF) - (24 - h) * 3_600_000).toISOString();

const bar = (
  from: string,
  lp: string | null,
  protocol: string | null,
  referral: string | null,
  split?: [string | null, string | null],
) => ({
  from,
  lpFeesUsd: lp,
  protocolFeesUsd: protocol,
  referralFeesUsd: referral,
  ...(split && { meteoraDammV2: { claimingFeesUsd: split[0], compoundingFeesUsd: split[1] } }),
});

const unsplit = { claiming: null, compounding: null };

describe("feeSlotsOf", () => {
  // Mutation: the response's step not read.
  it("draws the last hour on its twelve tranches", () => {
    const at = new Date(Date.parse(AS_OF) - 5 * 60_000).toISOString();
    const slots = feeSlotsOf(seriesOf("5m", AS_OF, [bar(at, "8", "1", "0")]));

    expect(slots).toHaveLength(12);
    expect(slots.at(-1)).toMatchObject({ lp: 8, protocol: 1, referral: 0 });
  });

  // Mutation: a part read from another's column — the protocol's fees drawn as the LPs'.
  it("carries each part of a bar from its own field", () => {
    const slots = feeSlotsOf(seriesOf("1h", AS_OF, [bar(hour(23), "80", "18", "2")]));

    expect(slots.at(-1)).toMatchObject({ lp: 80, protocol: 18, referral: 2 });
  });

  // Mutation: a slot without a bar read as zero — an hour not measured drawn as one without fees.
  it("keeps a slot without a bar as nothing, not zero", () => {
    const slots = feeSlotsOf(seriesOf("1h", AS_OF, [bar(hour(23), "80", "18", "2")]));

    expect(slots).toHaveLength(24);
    expect(slots.at(-2)).toMatchObject({ lp: null, protocol: null, referral: null });
  });
});

describe("feeTotalsOf and feeSharesOf", () => {
  // Mutation: a share taken over the LPs' part, or over the protocol's and the LPs' only — the
  // three shares no longer adding up to the whole.
  it("sums the period by part, each share over the three", () => {
    const slots = feeSlotsOf(seriesOf("1h", AS_OF, [bar(hour(22), "60", "15", "5"), bar(hour(23), "20", "0", "0")]));
    const totals = feeTotalsOf(slots);

    expect(totals).toEqual({ lp: 80, protocol: 15, referral: 5, ...unsplit });
    expect(totals && feeSharesOf(totals)).toEqual({ lp: 0.8, protocol: 0.15, referral: 0.05, ...unsplit });
  });

  // Mutation: the shares of a period without fees read as 0 % each — a split nobody paid.
  it("has no split for a period without fees", () => {
    expect(feeSharesOf({ lp: 0, protocol: 0, referral: 0, ...unsplit })).toEqual({
      lp: null,
      protocol: null,
      referral: null,
      ...unsplit,
    });
  });

  // Mutation: the totals started at zero — a period nothing valued read as one where nobody paid.
  it("has no totals for a period without a known bar", () => {
    const slots = feeSlotsOf(seriesOf("1h", AS_OF, [bar(hour(23), null, null, null)]));

    expect(feeTotalsOf(slots)).toBeNull();
  });

  // Mutation: an unknown part read as 0 — a fee not measured drawn and listed as one not paid.
  it("leaves an unknown part out of the sum, as unknown, not zero", () => {
    const slots = feeSlotsOf(seriesOf("1h", AS_OF, [bar(hour(23), "20", null, "5")]));
    const slot = slots.at(-1)!;

    expect(slot).toMatchObject({ lp: 20, protocol: null, referral: 5 });
    expect(feeSumOf(slot)).toBe(25);
    expect(feeSharesOf(slot)).toEqual({ lp: 0.8, protocol: null, referral: 0.2, ...unsplit });
    expect(feeTotalsOf(slots)).toEqual({ lp: 20, protocol: null, referral: 5, ...unsplit });
  });
});

describe("the LPs' split", () => {
  // Mutation: the LPs' whole summed beside its two halves — a split bar counted twice its LP fees.
  it("shows a bar's LP fees claimed and compounded in place of their whole", () => {
    const slots = feeSlotsOf(seriesOf("1h", AS_OF, [bar(hour(23), "80", "15", "5", ["60", "20"])]));
    const slot = slots.at(-1)!;

    expect(feePartsOf(slot)).toEqual(["claiming", "compounding", "protocol", "referral"]);
    expect(feeSumOf(slot)).toBe(100);
    expect(feeSharesOf(slot)).toEqual({ lp: null, claiming: 0.6, compounding: 0.2, protocol: 0.15, referral: 0.05 });
  });

  // Mutation: the period's split summed over the bars that know it — an hour measured before the
  // split read as one that compounded nothing.
  it("splits a period only if every bar with LP fees has its split", () => {
    const slots = feeSlotsOf(
      seriesOf("1h", AS_OF, [bar(hour(22), "40", "0", "0", [null, null]), bar(hour(23), "80", "15", "5", ["60", "20"])]),
    );
    const totals = feeTotalsOf(slots);

    expect(totals).toMatchObject({ lp: 120, claiming: null, compounding: null });
    expect(feePartsOf(totals)).toEqual(["lp", "protocol", "referral"]);
  });

  // Mutation: any bar without a split sinks the period's — a slot nothing was measured in, or an
  // unvalued one, read as a split missing.
  it("keeps the period's split over bars without LP fees", () => {
    const slots = feeSlotsOf(
      seriesOf("1h", AS_OF, [bar(hour(22), null, null, null, [null, null]), bar(hour(23), "80", "15", "5", ["60", "20"])]),
    );

    expect(feeTotalsOf(slots)).toMatchObject({ lp: 80, claiming: 60, compounding: 20 });
  });

  // Mutation: a bar without its block read as split — its LPs' fees dropped from the bar.
  it("keeps a bar without the block whole", () => {
    const slot = feeSlotsOf(seriesOf("1h", AS_OF, [bar(hour(23), "80", "15", "5")])).at(-1)!;

    expect(feePartsOf(slot)).toEqual(["lp", "protocol", "referral"]);
    expect(feeSumOf(slot)).toBe(100);
  });
});
