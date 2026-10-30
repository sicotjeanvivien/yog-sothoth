import { describe, expect, it } from "vitest";

import { OVERVIEW_PERIODS, pageQuery, parsePeriod, periodHeldClosed } from "../period";

describe("parsePeriod", () => {
  it("keeps an open period", () => {
    expect(parsePeriod("7d", OVERVIEW_PERIODS)).toBe("7d");
    expect(parsePeriod("1h", OVERVIEW_PERIODS)).toBe("1h");
  });

  // Mutation: the page's list not checked — a page reads a period it does not offer.
  it("falls back to 24h on a period the page does not offer", () => {
    expect(parsePeriod("1h", ["24h", "7d", "30d"])).toBe("24h");
  });

  it("falls back to 24h on an unknown or repeated value", () => {
    expect(parsePeriod("90d", OVERVIEW_PERIODS)).toBe("24h");
    expect(parsePeriod(["7d", "24h"], OVERVIEW_PERIODS)).toBe("24h");
  });
});

describe("periodHeldClosed", () => {
  const windows = [
    { window: "24h", open: true, opensAt: null },
    { window: "7d", open: false, opensAt: "2026-11-02T09:00:00Z" },
    { window: "30d", open: false, opensAt: null },
  ] as const;

  // Mutation: the date not read — a period opening soon sent to 24h, its blocks never saying when.
  it("holds a closed period with no date, not one that opens at a date, nor an open one", () => {
    expect(periodHeldClosed("30d", windows)).toBe(true);
    expect(periodHeldClosed("7d", windows)).toBe(false);
    expect(periodHeldClosed("24h", windows)).toBe(false);
  });
});

describe("pageQuery", () => {
  // Mutation: the default period written out gives one view two URLs.
  it("leaves the default period out and keeps the other parameters", () => {
    expect(pageQuery("24h", { tab: "fees" })).toEqual({ tab: "fees" });
    expect(pageQuery("7d", { tab: "fees" })).toEqual({ tab: "fees", period: "7d" });
  });
});
