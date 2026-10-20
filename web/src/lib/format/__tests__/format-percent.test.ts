import { describe, expect, it } from "vitest";

import { formatPercent, formatSignedPercent, formatTimes } from "../format-percent";

describe("formatSignedPercent", () => {
  it("shows the sign on a positive deviation", () => {
    expect(formatSignedPercent("0.0523", "en")).toBe("+5.2%");
  });

  it("keeps the sign on a negative deviation", () => {
    expect(formatSignedPercent("-0.2157", "en")).toBe("-21.6%");
  });

  it("localizes for fr", () => {
    // ⚠️ Loose on the space before %: U+00A0 or U+202F depending on the ICU build.
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

describe("formatTimes", () => {
  it("keeps two decimals, in the page's language", () => {
    expect(formatTimes(0.6812, "en")).toBe("× 0.68");
    expect(formatTimes(2, "fr")).toBe("× 2,00");
  });
});
