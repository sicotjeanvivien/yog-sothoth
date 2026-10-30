import { describe, expect, it } from "vitest";

import { feeSharesOf, feeSlotsOf, feeSumOf, feeTotalsOf } from "../fees";

const AS_OF = "2026-10-25T03:00:00Z";

const hour = (h: number) => new Date(Date.parse(AS_OF) - (24 - h) * 3_600_000).toISOString();

const bar = (from: string, lp: string | null, protocol: string | null, referral: string | null) => ({
  from,
  lpFeesUsd: lp,
  protocolFeesUsd: protocol,
  referralFeesUsd: referral,
});

describe("feeSlotsOf", () => {
  // Mutation: the period not passed through — the last hour drawn on 24 hourly slots.
  it("draws the last hour on its twelve tranches", () => {
    const at = new Date(Date.parse(AS_OF) - 5 * 60_000).toISOString();
    const slots = feeSlotsOf("1h", AS_OF, [bar(at, "8", "1", "0")]);

    expect(slots).toHaveLength(12);
    expect(slots.at(-1)).toMatchObject({ lp: 8, protocol: 1, referral: 0 });
  });

  // Mutation: a part read from another's column — the protocol's fees drawn as the LPs'.
  it("carries each part of a bar from its own field", () => {
    const slots = feeSlotsOf("24h", AS_OF, [bar(hour(23), "80", "18", "2")]);

    expect(slots.at(-1)).toMatchObject({ lp: 80, protocol: 18, referral: 2 });
  });

  // Mutation: a slot without a bar read as zero — an hour not measured drawn as one without fees.
  it("keeps a slot without a bar as nothing, not zero", () => {
    const slots = feeSlotsOf("24h", AS_OF, [bar(hour(23), "80", "18", "2")]);

    expect(slots).toHaveLength(24);
    expect(slots.at(-2)).toMatchObject({ lp: null, protocol: null, referral: null });
  });

  it("sums each part over the hours of a 7 days' slice", () => {
    const hours = Array.from({ length: 6 }, (_, h) =>
      bar(new Date(Date.parse("2026-10-24T21:00:00Z") + h * 3_600_000).toISOString(), "10", "2", "1"),
    );
    const slots = feeSlotsOf("7d", AS_OF, hours);

    expect(slots).toHaveLength(28);
    expect(slots.at(-1)).toMatchObject({ lp: 60, protocol: 12, referral: 6 });
  });
});

describe("feeTotalsOf and feeSharesOf", () => {
  // Mutation: a share taken over the LPs' part, or over the protocol's and the LPs' only — the
  // three shares no longer adding up to the whole.
  it("sums the period by part, each share over the three", () => {
    const slots = feeSlotsOf("24h", AS_OF, [bar(hour(22), "60", "15", "5"), bar(hour(23), "20", "0", "0")]);
    const totals = feeTotalsOf(slots);

    expect(totals).toEqual({ lp: 80, protocol: 15, referral: 5 });
    expect(totals && feeSharesOf(totals)).toEqual({ lp: 0.8, protocol: 0.15, referral: 0.05 });
  });

  // Mutation: the shares of a period without fees read as 0 % each — a split nobody paid.
  it("has no split for a period without fees", () => {
    expect(feeSharesOf({ lp: 0, protocol: 0, referral: 0 })).toEqual({
      lp: null,
      protocol: null,
      referral: null,
    });
  });

  // Mutation: the totals started at zero — a period nothing valued read as one where nobody paid.
  it("has no totals for a period without a known bar", () => {
    const slots = feeSlotsOf("24h", AS_OF, [bar(hour(23), null, null, null)]);

    expect(feeTotalsOf(slots)).toBeNull();
  });

  // Mutation: an unknown part read as 0 — a fee not measured drawn and listed as one not paid.
  it("leaves an unknown part out of the sum, as unknown, not zero", () => {
    const slots = feeSlotsOf("24h", AS_OF, [bar(hour(23), "20", null, "5")]);
    const slot = slots.at(-1)!;

    expect(slot).toMatchObject({ lp: 20, protocol: null, referral: 5 });
    expect(feeSumOf(slot)).toBe(25);
    expect(feeSharesOf(slot)).toEqual({ lp: 0.8, protocol: null, referral: 0.2 });
    expect(feeTotalsOf(slots)).toEqual({ lp: 20, protocol: null, referral: 5 });
  });
});
