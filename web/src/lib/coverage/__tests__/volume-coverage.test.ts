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
    // The measured n°1 of the ranking on 7 August 2026: $117 787 over 7 of the
    // 8 hours that traded, published as if it covered all 8.
    expect(
      volumeCoverage({ swapBuckets24h: 8, swapBucketsPriced24h: 7 }),
    ).toEqual({ priced: 7, total: 8, partial: true });
  });

  it("is partial when no hour at all could be valued", () => {
    // Such a pool is absent from the volume ranking entirely (`SUM` is NULL),
    // but it still appears in the `/pools` table with an empty figure — and
    // that emptiness has to be qualified too.
    expect(
      volumeCoverage({ swapBuckets24h: 3, swapBucketsPriced24h: 0 }),
    ).toEqual({ priced: 0, total: 3, partial: true });
  });

  it("separates two pools of equal volume and unequal coverage", () => {
    // The whole point of the ticket: same headline figure, different meaning.
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
