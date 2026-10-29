import { describe, expect, it } from "vitest";

import { isStale } from "../staleness";

const NOW = new Date("2026-10-20T16:00:00Z");

describe("isStale", () => {
  it("is not late at the normal lag of a frozen hour", () => {
    expect(isStale("2026-10-20T15:00:00Z", NOW, "24h")).toBe(false);
  });

  // Mutation: `>` turned `>=` raises the banner exactly at the threshold.
  it("is not late at exactly two hours", () => {
    expect(isStale("2026-10-20T14:00:00Z", NOW, "24h")).toBe(false);
  });

  // Mutation: the threshold raised to a day keeps a stopped writer silent.
  it("is late past two hours", () => {
    expect(isStale("2026-10-20T13:59:00Z", NOW, "24h")).toBe(true);
  });

  // Mutation: the frozen hours' threshold over the last hour — a writer stopped for 1 h 45 stays silent.
  it("is late over the last hour past half an hour, not before", () => {
    expect(isStale("2026-10-20T15:31:00Z", NOW, "1h")).toBe(false);
    expect(isStale("2026-10-20T15:29:00Z", NOW, "1h")).toBe(true);
  });

  it("is never late before the first freeze", () => {
    expect(isStale(null, NOW, "24h")).toBe(false);
  });
});
