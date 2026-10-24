import { describe, expect, it } from "vitest";

import { pageQuery, parsePeriod } from "../period";

describe("parsePeriod", () => {
  it("keeps an open period", () => {
    expect(parsePeriod("7d")).toBe("7d");
  });

  // Mutation: the `CLOSED_PERIODS` guard dropped runs 30d's reads from its URL.
  it("falls back to 24h on a closed period", () => {
    expect(parsePeriod("30d")).toBe("24h");
  });

  it("falls back to 24h on an unknown or repeated value", () => {
    expect(parsePeriod("1h")).toBe("24h");
    expect(parsePeriod(["7d", "24h"])).toBe("24h");
  });
});

describe("pageQuery", () => {
  // Mutation: the default period written out gives one view two URLs.
  it("leaves the default period out and keeps the other parameters", () => {
    expect(pageQuery("24h", { tab: "fees" })).toEqual({ tab: "fees" });
    expect(pageQuery("7d", { tab: "fees" })).toEqual({ tab: "fees", period: "7d" });
  });
});
