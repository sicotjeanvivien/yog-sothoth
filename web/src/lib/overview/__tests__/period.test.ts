import { describe, expect, it } from "vitest";

import { coverageUnit, OVERVIEW_PERIODS, pageQuery, parsePeriod } from "../period";

describe("parsePeriod", () => {
  it("keeps an open period", () => {
    expect(parsePeriod("7d", OVERVIEW_PERIODS)).toBe("7d");
    expect(parsePeriod("1h", OVERVIEW_PERIODS)).toBe("1h");
  });

  // Mutation: the page's list not checked — a page reads a period it does not offer.
  it("falls back to 24h on a period the page does not offer", () => {
    expect(parsePeriod("1h", ["24h", "7d", "30d"])).toBe("24h");
  });

  // Mutation: the `CLOSED_PERIODS` guard dropped runs 30d's reads from its URL.
  it("falls back to 24h on a closed period", () => {
    expect(parsePeriod("30d", OVERVIEW_PERIODS)).toBe("24h");
  });

  it("falls back to 24h on an unknown or repeated value", () => {
    expect(parsePeriod("90d", OVERVIEW_PERIODS)).toBe("24h");
    expect(parsePeriod(["7d", "24h"], OVERVIEW_PERIODS)).toBe("24h");
  });
});

describe("pageQuery", () => {
  // Mutation: the default period written out gives one view two URLs.
  it("leaves the default period out and keeps the other parameters", () => {
    expect(pageQuery("24h", { tab: "fees" })).toEqual({ tab: "fees" });
    expect(pageQuery("7d", { tab: "fees" })).toEqual({ tab: "fees", period: "7d" });
  });
});

describe("coverageUnit", () => {
  // Mutation: `<` turned `<=` — the hours' coverage worded as tranches.
  it("counts tranches under an hour's bars, pool-hours from an hour", () => {
    expect(coverageUnit("1h")).toBe("poolTranches");
    expect(coverageUnit("24h")).toBe("poolHours");
  });
});
