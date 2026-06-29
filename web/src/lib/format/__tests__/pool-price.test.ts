import { describe, expect, it } from "vitest";

import { formatPrice } from "../pool-price";

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
