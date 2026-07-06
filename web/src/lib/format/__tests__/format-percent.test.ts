import { describe, expect, it } from "vitest";

import { formatPercent, formatSignedPercent } from "../format-percent";

describe("formatSignedPercent", () => {
  it("shows the sign on a positive deviation", () => {
    expect(formatSignedPercent("0.0523", "en")).toBe("+5.2%");
  });

  it("keeps the sign on a negative deviation", () => {
    expect(formatSignedPercent("-0.2157", "en")).toBe("-21.6%");
  });

  it("localizes for fr", () => {
    // fr-FR: comma decimal, and a non-breaking space before % \u2014 its
    // exact codepoint (U+00A0 vs U+202F) varies with the ICU build, so
    // the assertion stays loose on that character.
    expect(formatSignedPercent("0.0523", "fr")).toMatch(/^\+5,2\s%$/u);
  });

  it("returns a non-numeric string as-is instead of NaN", () => {
    expect(formatSignedPercent("not-a-ratio", "en")).toBe("not-a-ratio");
  });
});

describe("formatPercent", () => {
  it("drops the sign — magnitude only", () => {
    expect(formatPercent("-0.78", "en")).toBe("78%");
  });

  it("renders a plain threshold", () => {
    expect(formatPercent("0.0500", "en")).toBe("5%");
  });
});
