import { describe, expect, it } from "vitest";
import { formatUsd, formatUsdShares } from "../format-usd";

describe("formatUsd", () => {
  it("formats a plain amount to the cent", () => {
    expect(formatUsd("1234.5")).toBe("$1,234.50");
  });

  it("renders an em-dash when the amount is unknown", () => {
    expect(formatUsd(null)).toBe("—");
    expect(formatUsd(undefined)).toBe("—");
    expect(formatUsd("not a number")).toBe("—");
  });
});

describe("formatUsdShares", () => {
  it("makes the displayed shares add up to the displayed total", () => {
    // The real figures from the SOL/USDC sheet. Rounded one by one they read
    // $0.38 + $0.10 + $1.43 = $1.91, under a total displayed as $1.90.
    const naive = ["0.3800", "0.0950", "1.4250"].map(formatUsd);
    expect(centsOf(naive)).toBe(191);
    expect(centsOf([formatUsd("1.9000")])).toBe(190);

    const shares = formatUsdShares("1.9000", ["0.3800", "0.0950", "1.4250"]);

    expect(shares).toEqual(["$0.38", "$0.10", "$1.42"]);
    expect(centsOf(shares)).toBe(190);
  });

  it("gives the leftover cents to the largest discarded fractions", () => {
    // Three shares of $1.00, each $0.3333…: two cents to hand out, and every
    // fraction is equal, so the tie-break keeps the declared order.
    const shares = formatUsdShares("1.00", [
      "0.333333",
      "0.333333",
      "0.333334",
    ]);

    expect(shares).toEqual(["$0.33", "$0.33", "$0.34"]);
    expect(centsOf(shares)).toBe(100);
  });

  it("leaves exact shares untouched", () => {
    expect(formatUsdShares("103.00", ["21.63", "5.15", "76.22"])).toEqual([
      "$21.63",
      "$5.15",
      "$76.22",
    ]);
  });

  it("does NOT invent additivity when the shares do not partition the total", () => {
    // The guard that matters: a formatter silently balancing the rows would
    // hide a real API defect behind a plausible display. $1 + $1 is not $10,
    // and the page must keep saying so.
    expect(formatUsdShares("10.00", ["1.00", "1.00"])).toEqual([
      "$1.00",
      "$1.00",
    ]);
  });

  it("falls back to plain formatting when the total is unknown", () => {
    expect(formatUsdShares(null, ["1.005", "2.005"])).toEqual([
      "$1.01",
      "$2.01",
    ]);
  });

  it("falls back to plain formatting when any share is unknown", () => {
    // "we don't know" cannot be balanced against — the remaining shares keep
    // their own rounding rather than absorbing a phantom remainder.
    expect(formatUsdShares("3.00", ["1.005", null])).toEqual(["$1.01", "—"]);
  });

  it("handles a zero total without handing out phantom cents", () => {
    expect(formatUsdShares("0", ["0", "0"])).toEqual(["$0.00", "$0.00"]);
  });
});

/** Sum of formatted dollar strings, in cents. */
function centsOf(formatted: string[]): number {
  return formatted.reduce(
    (acc, s) => acc + Math.round(Number.parseFloat(s.replace(/[$,]/g, "")) * 100),
    0,
  );
}
