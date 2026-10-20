import { describe, expect, it } from "vitest";

import { volumeCoverage } from "../volume-coverage";

describe("volumeCoverage", () => {
  it("returns null when the pool did not trade", () => {
    expect(
      volumeCoverage({ poolHours: 0, poolHoursValued: 0 }),
    ).toBeNull();
  });

  it("is not partial when every hour that traded could be valued", () => {
    expect(
      volumeCoverage({ poolHours: 8, poolHoursValued: 8 }),
    ).toEqual({ priced: 8, total: 8, partial: false, share: 1 });
  });

  it("is partial when one hour could not be valued", () => {
    expect(
      volumeCoverage({ poolHours: 8, poolHoursValued: 7 }),
    ).toEqual({ priced: 7, total: 8, partial: true, share: 0.87 });
  });

  it("is partial when no hour at all could be valued", () => {
    expect(
      volumeCoverage({ poolHours: 3, poolHoursValued: 0 }),
    ).toEqual({ priced: 0, total: 3, partial: true, share: 0 });
  });

  // Mutation: rounding to nearest shows a partial coverage as "100 %", a complete one.
  it("never reads a partial coverage as whole", () => {
    expect(volumeCoverage({ poolHours: 1000, poolHoursValued: 999 })?.share).toBe(0.99);
  });

  it("separates two pools of equal volume and unequal coverage", () => {
    const measured = volumeCoverage({
      poolHours: 4,
      poolHoursValued: 4,
    });
    const subTotal = volumeCoverage({
      poolHours: 4,
      poolHoursValued: 2,
    });

    expect(measured?.partial).toBe(false);
    expect(subTotal?.partial).toBe(true);
    expect(measured).not.toEqual(subTotal);
  });
});
