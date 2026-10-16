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
    // Rounded one by one: $0.38 + $0.10 + $1.43 = $1.91 under a $1.90 total.
    const naive = ["0.3800", "0.0950", "1.4250"].map(formatUsd);
    expect(centsOf(naive)).toBe(191);
    expect(centsOf([formatUsd("1.9000")])).toBe(190);

    const shares = formatUsdShares("1.9000", ["0.3800", "0.0950", "1.4250"]);

    expect(shares).toEqual(["$0.38", "$0.10", "$1.42"]);
    expect(centsOf(shares)).toBe(190);
  });

  it("gives the leftover cent to the largest discarded fraction", () => {
    // One cent to hand out, to the biggest discarded fraction (.3334 against .3333).
    const shares = formatUsdShares("1.00", [
      "0.333333",
      "0.333333",
      "0.333334",
    ]);

    expect(shares).toEqual(["$0.33", "$0.33", "$0.34"]);
    expect(centsOf(shares)).toBe(100);
  });

  it("breaks a tie on the declared order, so the output is stable", () => {
    // Mutation: a non-deterministic tie-break between two exact halves.
    expect(formatUsdShares("0.01", ["0.005", "0.005"])).toEqual([
      "$0.01",
      "$0.00",
    ]);
  });

  it("balances against the cents the total is DISPLAYED as", () => {
    // Mutation: targeting `total * 100`, which rounds down in binary, under a `$0.15` total.
    const total = formatUsd("0.145");
    const shares = formatUsdShares("0.145", ["0.100", "0.045"]);

    expect(total).toBe("$0.15");
    expect(centsOf(shares)).toBe(centsOf([total]));
  });

  it("leaves exact shares untouched", () => {
    expect(formatUsdShares("103.00", ["21.63", "5.15", "76.22"])).toEqual([
      "$21.63",
      "$5.15",
      "$76.22",
    ]);
  });

  it("does NOT invent additivity when the shares do not partition the total", () => {
    // Mutation: balancing shares that do not sum to the total.
    expect(formatUsdShares("10.00", ["1.00", "1.00"])).toEqual([
      "$1.00",
      "$1.00",
    ]);
  });

  it("catches a sub-cent non-partition, not just a gross one", () => {
    // Mutation: a half-cent tolerance, which would balance real drift into `$5.01 + $4.99`.
    expect(formatUsdShares("10.00", ["5.004", "4.992"])).toEqual([
      "$5.00",
      "$4.99",
    ]);
  });

  it("falls back to plain formatting when the total is unknown", () => {
    expect(formatUsdShares(null, ["1.005", "2.005"])).toEqual([
      "$1.01",
      "$2.01",
    ]);
  });

  it("falls back to plain formatting when any share is unknown", () => {
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
