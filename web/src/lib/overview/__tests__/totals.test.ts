import { describe, expect, it } from "vitest";

import { amountOf, changeRatio, shareOf, sumUsd, swapsPerSecond } from "../totals";

describe("amountOf", () => {
  it("reads the API's decimal string", () => {
    expect(amountOf("1.5")).toBe(1.5);
  });

  // Mutation: an absent total read as 0 renders a "+0 %" instead of no figure.
  it("is unknown on an absent or unreadable value", () => {
    expect(amountOf(null)).toBeNull();
    expect(amountOf(undefined)).toBeNull();
    expect(amountOf("n/a")).toBeNull();
  });
});

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

describe("shareOf", () => {
  it("is the part over the whole", () => {
    expect(shareOf(18, 100)).toBeCloseTo(0.18);
  });

  // Mutation: the zero guard dropped renders a fee rate of "∞ %" on a period without volume.
  it("is unknown on a zero whole, or an unknown term", () => {
    expect(shareOf(1, 0)).toBeNull();
    expect(shareOf(null, 10)).toBeNull();
    expect(shareOf(1, null)).toBeNull();
  });
});

describe("swapsPerSecond", () => {
  // Mutation: dividing by hours, or by the 24h window on 7d, misstates the rate.
  it("spreads the swaps over the period's seconds", () => {
    expect(swapsPerSecond(86_400 * 39, "24h")).toBe(39);
    expect(swapsPerSecond(604_800 * 39, "7d")).toBe(39);
    expect(swapsPerSecond(3_600 * 39, "1h")).toBe(39);
  });
});
