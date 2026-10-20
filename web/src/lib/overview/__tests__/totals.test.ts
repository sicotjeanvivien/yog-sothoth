import { describe, expect, it } from "vitest";

import { changeRatio, rotation, sumUsd, swapsPerSecond } from "../totals";

describe("sumUsd", () => {
  it("adds the protocols' amounts", () => {
    expect(sumUsd(["1.5", "2"])).toBe(3.5);
  });

  // Mutation: a null skipped publishes one protocol's amount as the total.
  it("is unknown when one term is", () => {
    expect(sumUsd(["1.5", null])).toBeNull();
  });

  it("is unknown on no row at all, not zero", () => {
    expect(sumUsd([])).toBeNull();
  });
});

describe("changeRatio", () => {
  it("is the relative change", () => {
    expect(changeRatio(110, 100)).toBeCloseTo(0.1);
  });

  // Mutation: the zero guard dropped renders "+∞ %".
  it("is unknown on a zero base", () => {
    expect(changeRatio(10, 0)).toBeNull();
  });
});

describe("rotation", () => {
  // Mutation: TVL over volume renders × 2 for a liquidity that turned half over.
  it("is the volume over the TVL", () => {
    expect(rotation(50, 100)).toBe(0.5);
  });

  // Mutation: the zero guard dropped renders "× ∞".
  it("is unknown on a zero TVL", () => {
    expect(rotation(50, 0)).toBeNull();
  });

  it("is unknown when the volume is", () => {
    expect(rotation(null, 100)).toBeNull();
  });
});

describe("swapsPerSecond", () => {
  // Mutation: dividing by hours, or by the 24h window on 7d, misstates the rate.
  it("spreads the swaps over the period's seconds", () => {
    expect(swapsPerSecond(86_400 * 39, "24h")).toBe(39);
    expect(swapsPerSecond(604_800 * 39, "7d")).toBe(39);
  });
});
