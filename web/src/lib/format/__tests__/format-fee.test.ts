import { describe, expect, it } from "vitest";
import { formatComputedFeeBps, formatFeeBps, formatFeeSplit } from "../format-fee";

const LABELS = { protocol: "Protocol", referral: "Referral" };

describe("formatFeeBps", () => {
  it("formats a standard tier as a percentage", () => {
    expect(formatFeeBps("25")).toBe("0.25%");
  });

  it("trims trailing zeros on a whole-percent tier", () => {
    expect(formatFeeBps("100")).toBe("1%");
  });

  it("keeps the anti-sniper cliff readable", () => {
    expect(formatFeeBps("5000")).toBe("50%");
  });

  it("preserves a fractional (sub-bps) fee", () => {
    expect(formatFeeBps("2.5")).toBe("0.025%");
  });

  it("renders an em-dash when the fee is unknown", () => {
    expect(formatFeeBps(null)).toBe("—");
  });

  it("renders an em-dash on a non-numeric value", () => {
    expect(formatFeeBps("not-a-number")).toBe("—");
  });
});

describe("formatFeeSplit", () => {
  it("joins the two labeled percents", () => {
    expect(formatFeeSplit(20, 20, LABELS)).toBe("Protocol 20% · Referral 20%");
  });

  it("renders an em-dash when either percent is unknown", () => {
    expect(formatFeeSplit(null, 20, LABELS)).toBe("—");
    expect(formatFeeSplit(20, null, LABELS)).toBe("—");
  });
});

describe("formatComputedFeeBps", () => {
  // The PR's headline case: 28BDU1…'s linear floor is numerator 40_000_064,
  // i.e. 400.00064 bps. `formatFeeBps` renders that as "4.000006%", which reads
  // as a glitch beside a tidy "50%" tier.
  it("rounds a decayed fee instead of showing the chain's last digits", () => {
    expect(formatComputedFeeBps("400.00064")).toBe("4%");
    expect(formatFeeBps("400.00064")).toBe("4.000006%");
  });

  it("keeps a basis point of resolution", () => {
    expect(formatComputedFeeBps("25.39394")).toBe("0.25%");
    expect(formatComputedFeeBps("1234.5")).toBe("12.35%");
  });

  it("is the dash when the fee could not be established", () => {
    expect(formatComputedFeeBps(null)).toBe("—");
    expect(formatComputedFeeBps("not-a-number")).toBe("—");
  });
});
