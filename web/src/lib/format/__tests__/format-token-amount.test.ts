import { describe, expect, it } from "vitest";

import { formatTokenAmount } from "../format-token-amount";

describe("formatTokenAmount", () => {
  // Mutation: the raw amount shown without its decimals — 1.48K DINU read as 1.48T.
  it("divides a raw amount by its decimals", () => {
    expect(formatTokenAmount("1480353083770", 9)).toBe("1.48K");
    expect(formatTokenAmount("2500000", 6)).toBe("2.5");
  });

  it("keeps an amount under one unit readable", () => {
    expect(formatTokenAmount("1234", 6)).toBe("0.00123");
    expect(formatTokenAmount("0", 9)).toBe("0");
  });

  it("reads a whole u128 without losing its order of magnitude", () => {
    expect(formatTokenAmount("340282366920938463463374607431768211455", 18)).toBe("340,282,366.92T");
  });

  it("takes a token with 0 decimals as whole units", () => {
    expect(formatTokenAmount("42", 0)).toBe("42");
  });

  // Mutation: a fallback to 0 decimals — the raw amount passes for units of the token.
  it("shows no amount while the decimals are unknown, or the raw is not a count", () => {
    expect(formatTokenAmount("1480353083770", null)).toBeNull();
    expect(formatTokenAmount("-5", 6)).toBeNull();
  });
});
