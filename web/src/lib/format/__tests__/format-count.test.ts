import { describe, expect, it } from "vitest";
import { formatCount, formatCountCompact, formatRate } from "../format-count";

describe("formatCount", () => {
  it("renders a small count verbatim", () => {
    expect(formatCount(359)).toBe("359");
  });

  it("groups thousands with a comma", () => {
    expect(formatCount(12500)).toBe("12,500");
  });

  it("renders zero", () => {
    expect(formatCount(0)).toBe("0");
  });

  it("renders an em-dash when the count is null or undefined", () => {
    expect(formatCount(null)).toBe("—");
    expect(formatCount(undefined)).toBe("—");
  });

  it("renders an em-dash on a non-finite value", () => {
    expect(formatCount(Number.NaN)).toBe("—");
  });
});

describe("formatCountCompact", () => {
  it("shortens millions to two decimals", () => {
    expect(formatCountCompact(3_410_000)).toBe("3.41M");
  });

  it("renders an em-dash when the count is null", () => {
    expect(formatCountCompact(null)).toBe("—");
  });
});

describe("formatRate", () => {
  it("is whole from 10 per second", () => {
    expect(formatRate(38.6)).toBe("39");
  });

  // Mutation: a fixed decimal reads a slow but real pace as "0.0".
  it("keeps a slow pace above zero", () => {
    expect(formatRate(0.012)).toBe("0.012");
    expect(formatRate(3.44)).toBe("3.4");
  });
});
