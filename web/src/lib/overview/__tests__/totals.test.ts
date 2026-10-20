import { describe, expect, it } from "vitest";

import { changeRatio, sumUsd } from "../totals";

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
