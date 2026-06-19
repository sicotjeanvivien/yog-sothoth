import { describe, expect, it } from "vitest";

import { computePoolPrice, formatPrice } from "../pool-price";

describe("computePoolPrice", () => {
  it("derives the A↔B rate from decimal-adjusted reserves", () => {
    // 100 SOL (9 decimals) vs 15234 USDC (6 decimals) → 152.34 USDC per SOL.
    const price = computePoolPrice({
      reserveA: "100000000000", // 100 * 1e9
      reserveB: "15234000000", // 15234 * 1e6
      decimalsA: 9,
      decimalsB: 6,
    });
    expect(price).not.toBeNull();
    expect(price?.priceAInB).toBeCloseTo(152.34, 6);
    expect(price?.priceBInA).toBeCloseTo(1 / 152.34, 10);
  });

  it("is reciprocal-consistent", () => {
    const price = computePoolPrice({
      reserveA: "100000000000",
      reserveB: "15234000000",
      decimalsA: 9,
      decimalsB: 6,
    });
    expect((price?.priceAInB ?? 0) * (price?.priceBInA ?? 0)).toBeCloseTo(1, 10);
  });

  it("returns null when a reserve is zero (no defined price)", () => {
    expect(
      computePoolPrice({ reserveA: "0", reserveB: "15234000000", decimalsA: 9, decimalsB: 6 }),
    ).toBeNull();
    expect(
      computePoolPrice({ reserveA: "100000000000", reserveB: "0", decimalsA: 9, decimalsB: 6 }),
    ).toBeNull();
  });

  it("returns null on a non-numeric reserve", () => {
    expect(
      computePoolPrice({ reserveA: "nope", reserveB: "15234000000", decimalsA: 9, decimalsB: 6 }),
    ).toBeNull();
  });
});

describe("formatPrice", () => {
  it("uses two fraction digits for normal prices", () => {
    expect(formatPrice(152.3399)).toBe("152.34");
  });

  it("uses compact notation above 1000", () => {
    expect(formatPrice(1_250_000)).toBe("1.25M");
  });

  it("keeps significant digits for sub-1 prices", () => {
    expect(formatPrice(0.0065723)).toBe("0.006572");
  });

  it("floors tiny prices to a readable bound", () => {
    expect(formatPrice(1e-12)).toBe("< 0.00000001");
  });

  it("renders an em-dash for non-finite or non-positive input", () => {
    expect(formatPrice(Number.NaN)).toBe("—");
    expect(formatPrice(0)).toBe("—");
    expect(formatPrice(-5)).toBe("—");
  });
});
