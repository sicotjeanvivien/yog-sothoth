import { describe, expect, it } from "vitest";

import { CURVE_BOX, curveOf, flowSlotsOf } from "../liquidity";

const tvl = (at: string, tvlUsd: string | null) => ({ at, tvlUsd });

describe("curveOf", () => {
  // Mutation: the axis from zero — a move of 1 % on $18 M would draw a flat line.
  it("spans the axis from the lowest to the highest TVL", () => {
    const curve = curveOf([
      tvl("2026-10-24T01:00:00Z", "17750000"),
      tvl("2026-10-24T02:00:00Z", "18300000"),
    ]);

    expect(curve?.low).toBe(17_750_000);
    expect(curve?.high).toBe(18_300_000);
    expect(curve?.coords[0]?.y).toBeGreaterThan(curve?.coords[1]?.y ?? Infinity);
    expect(curve?.coords.at(-1)?.x).toBe(CURVE_BOX.width);
  });

  // Mutation: a point without a value drawn at zero.
  it("leaves out a point without a value", () => {
    const curve = curveOf([
      tvl("2026-10-24T01:00:00Z", "10"),
      tvl("2026-10-24T02:00:00Z", null),
      tvl("2026-10-24T03:00:00Z", "12"),
    ]);

    expect(curve?.points.map((p) => p.value)).toEqual([10, 12]);
  });

  it("is nothing under two points", () => {
    expect(curveOf([tvl("2026-10-24T01:00:00Z", "10")])).toBeNull();
  });
});

describe("flowSlotsOf", () => {
  // Mutation: the period not passed through — the last hour drawn on 24 hourly slots.
  it("draws the last hour on its twelve tranches", () => {
    const at = new Date(Date.parse("2026-10-24T12:00:00Z") - 5 * 60_000).toISOString();
    const slots = flowSlotsOf("1h", "2026-10-24T12:00:00Z", [
      { from: at, createdUsd: "0", addedUsd: "4", removedUsd: "1" },
    ]);

    expect(slots).toHaveLength(12);
    expect(slots.at(-1)).toMatchObject({ created: 0, added: 4, removed: 1 });
  });

  // Mutation: an hour without a bar read as zero — an hour before the liquidity was measured
  // drawn as one without any move.
  it("keeps a slot without a bar as nothing, not zero", () => {
    const slots = flowSlotsOf("24h", "2026-10-24T12:00:00Z", [
      { from: "2026-10-24T11:00:00Z", createdUsd: "7", addedUsd: "5", removedUsd: "2" },
    ]);

    expect(slots).toHaveLength(24);
    expect(slots.at(-2)).toMatchObject({ created: null, added: null, removed: null });
    // Mutation: the creations read from the added column — a creation drawn as an addition.
    expect(slots.at(-1)).toMatchObject({ created: 7, added: 5, removed: 2 });
  });

  // Mutation: each flow kept from one hour of the slice — a 6-hour bar worth its last hour.
  it("sums each flow over the hours of a 7 days' slice", () => {
    const hours = Array.from({ length: 6 }, (_, h) => ({
      from: new Date(Date.parse("2026-10-24T06:00:00Z") + h * 3_600_000).toISOString(),
      createdUsd: "1",
      addedUsd: "2",
      removedUsd: "3",
    }));
    const slots = flowSlotsOf("7d", "2026-10-24T12:00:00Z", hours);

    expect(slots).toHaveLength(28);
    expect(slots.at(-1)).toMatchObject({ created: 6, added: 12, removed: 18 });
  });
});
