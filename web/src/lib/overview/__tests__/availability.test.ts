import { describe, expect, it } from "vitest";

import {
  periodState,
  previousWindowCovered,
} from "../availability";

const FIRST_HOUR = "2026-10-13T13:00:00Z";

describe("periodState", () => {
  // Mutation: `>=` turned `>` closes 7d on the very hour its facts become whole.
  it("opens 7d once the facts span exactly seven days", () => {
    expect(
      periodState("7d", {
        from: null,
        asOf: "2026-10-20T13:00:00Z",
        coveredSince: FIRST_HOUR,
      }),
    ).toEqual({ open: true });
  });

  // Mutation: the comparison dropped, every period opens on a partial sum.
  it("closes 7d an hour short, and says when it opens", () => {
    expect(
      periodState("7d", {
        from: null,
        asOf: "2026-10-20T12:00:00Z",
        coveredSince: FIRST_HOUR,
      }),
    ).toEqual({ open: false, opensAt: new Date("2026-10-20T13:00:00Z") });
  });

  it("opens 24h on a day of facts", () => {
    expect(
      periodState("24h", {
        from: null,
        asOf: "2026-10-14T13:00:00Z",
        coveredSince: FIRST_HOUR,
      }),
    ).toEqual({ open: true });
  });

  // Mutation: `CLOSED_PERIODS` emptied opens 30d on thirty days of facts.
  it("keeps 30d closed even when its facts are whole", () => {
    expect(
      periodState("30d", {
        from: null,
        asOf: "2026-12-01T00:00:00Z",
        coveredSince: FIRST_HOUR,
      }),
    ).toEqual({ open: false, opensAt: null });
  });

  it("closes every period before the first freeze", () => {
    expect(
      periodState("24h", { from: null, asOf: null, coveredSince: null }),
    ).toEqual({ open: false, opensAt: null });
  });
});

describe("previousWindowCovered", () => {
  // Mutation: `>=` turned `>` hides the variation of a window starting on the first hour.
  it("is covered when the previous window starts on the first hour", () => {
    expect(
      previousWindowCovered({
        from: FIRST_HOUR,
        asOf: "2026-10-20T13:00:00Z",
        coveredSince: FIRST_HOUR,
      }),
    ).toBe(true);
  });

  // Mutation: the comparison dropped, a variation compares to a partial sum.
  it("is not covered when the previous window starts an hour earlier", () => {
    expect(
      previousWindowCovered({
        from: "2026-10-13T12:00:00Z",
        asOf: "2026-10-20T12:00:00Z",
        coveredSince: FIRST_HOUR,
      }),
    ).toBe(false);
  });

  it("is not covered before the first freeze", () => {
    expect(
      previousWindowCovered({ from: null, asOf: null, coveredSince: null }),
    ).toBe(false);
  });
});
