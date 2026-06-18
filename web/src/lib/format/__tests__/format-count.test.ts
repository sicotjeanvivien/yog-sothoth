import { describe, expect, it } from "vitest";
import { formatCount } from "../format-count";

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
