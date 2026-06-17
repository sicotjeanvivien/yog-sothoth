import { describe, expect, it } from "vitest";
import { formatFeeBps, formatFeeSplit } from "../format-fee";

const LABELS = { protocol: "Protocol", partner: "Partner", referral: "Referral" };

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
  it("joins the three labeled percents", () => {
    expect(formatFeeSplit(20, 0, 20, LABELS)).toBe(
      "Protocol 20% · Partner 0% · Referral 20%",
    );
  });

  it("renders an em-dash when any percent is unknown", () => {
    expect(formatFeeSplit(null, 0, 20, LABELS)).toBe("—");
    expect(formatFeeSplit(20, null, 20, LABELS)).toBe("—");
    expect(formatFeeSplit(20, 0, null, LABELS)).toBe("—");
  });
});
