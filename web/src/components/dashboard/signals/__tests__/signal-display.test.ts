import { describe, expect, it } from "vitest";

import type { SignalResponse } from "@/lib/api/schema/zod.gen";

import { formatSignalThreshold, formatSignalValue, leanedSymbolOf } from "../signal-display";

function signal(overrides: Partial<SignalResponse>): SignalResponse {
  const token = { mint: null, name: null, decimals: 0, logoUri: null };
  return {
    id: 1,
    detector: "later_detector",
    protocol: "meteora_damm_v2",
    poolAddress: "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1",
    tokenA: { ...token, symbol: "SOL" },
    tokenB: { ...token, symbol: "USDC" },
    severity: "warning",
    value: "0.75",
    threshold: "0.6",
    valueUnit: "raw",
    leansToward: null,
    message: null,
    triggeredAt: "2026-07-02T10:00:00Z",
    ...overrides,
  };
}

describe("formatSignalValue", () => {
  // Mutation: the sign dropped hides which way a deviation goes.
  it("signs a signed ratio", () => {
    expect(formatSignalValue(signal({ valueUnit: "signedRatio" }), "en")).toBe("+75%");
  });

  // Mutation: a sign on a drain reads as growth.
  it("leaves a ratio unsigned", () => {
    expect(formatSignalValue(signal({ valueUnit: "ratio" }), "en")).toBe("75%");
  });

  // Mutation: a detector's own unit read as a ratio.
  it("shows a raw value as it is served", () => {
    expect(formatSignalValue(signal({ valueUnit: "raw" }), "en")).toBe("0.75");
  });

  // Mutation: the unit read off the detector's name again.
  it("reads the unit, not the detector", () => {
    expect(formatSignalValue(signal({ detector: "tvl_drain", valueUnit: "raw" }), "en")).toBe("0.75");
  });
});

describe("formatSignalThreshold", () => {
  // Mutation: the threshold formatted as the value gives it a "+"; a drain's left raw.
  it("has no sign, whatever the value's", () => {
    expect(formatSignalThreshold(signal({ valueUnit: "signedRatio" }), "en")).toBe("60%");
    expect(formatSignalThreshold(signal({ valueUnit: "ratio" }), "en")).toBe("60%");
  });

  it("stays raw in a raw unit, and is empty without a threshold", () => {
    expect(formatSignalThreshold(signal({ valueUnit: "raw" }), "en")).toBe("0.6");
    expect(formatSignalThreshold(signal({ threshold: null }), "en")).toBe("—");
  });
});

describe("leanedSymbolOf", () => {
  // Mutation: the two tokens swapped sends the sentence to the token the swaps left.
  it("names the token the API says", () => {
    expect(leanedSymbolOf(signal({ leansToward: "tokenA" }))).toBe("SOL");
    expect(leanedSymbolOf(signal({ leansToward: "tokenB" }))).toBe("USDC");
  });

  it("names none where the API says none", () => {
    expect(leanedSymbolOf(signal({ leansToward: null, value: "-0.75" }))).toBeNull();
  });

  // Mutation: a fallback on the other token names one the swaps left.
  it("names none while the token is unresolved", () => {
    const tokenA = { mint: null, name: null, decimals: 0, logoUri: null, symbol: null };
    expect(leanedSymbolOf(signal({ leansToward: "tokenA", tokenA }))).toBeNull();
  });
});
