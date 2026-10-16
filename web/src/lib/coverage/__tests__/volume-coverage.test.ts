import { describe, expect, it } from "vitest";

import { volumeCoverage } from "../volume-coverage";

describe("volumeCoverage", () => {
  it("returns null when the pool did not trade", () => {
    expect(
      volumeCoverage({ swapBuckets24h: 0, swapBucketsPriced24h: 0 }),
    ).toBeNull();
  });

  it("is not partial when every hour that traded could be valued", () => {
    expect(
      volumeCoverage({ swapBuckets24h: 8, swapBucketsPriced24h: 8 }),
    ).toEqual({ priced: 8, total: 8, partial: false });
  });

  it("is partial when one hour could not be valued", () => {
    expect(
      volumeCoverage({ swapBuckets24h: 8, swapBucketsPriced24h: 7 }),
    ).toEqual({ priced: 7, total: 8, partial: true });
  });

  it("is partial when no hour at all could be valued", () => {
    // Absent from the ranking, but shown in `/pools` with an empty figure that needs qualifying.
    expect(
      volumeCoverage({ swapBuckets24h: 3, swapBucketsPriced24h: 0 }),
    ).toEqual({ priced: 0, total: 3, partial: true });
  });

  it("separates two pools of equal volume and unequal coverage", () => {
    const measured = volumeCoverage({
      swapBuckets24h: 4,
      swapBucketsPriced24h: 4,
    });
    const subTotal = volumeCoverage({
      swapBuckets24h: 4,
      swapBucketsPriced24h: 2,
    });

    expect(measured?.partial).toBe(false);
    expect(subTotal?.partial).toBe(true);
    expect(measured).not.toEqual(subTotal);
  });
});
