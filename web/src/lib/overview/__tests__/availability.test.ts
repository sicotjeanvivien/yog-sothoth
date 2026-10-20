import { describe, expect, it } from "vitest";

import {
  periodState,
  previousWindowCovered,
  tvlStart,
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

describe("tvlStart", () => {
  const point = (protocol: "meteora_damm_v2" | "meteora_dlmm", at: string) => ({
    protocol,
    at,
    tvlUsd: "1000",
    poolsPriced: 1,
  });

  it("is the point measured at the window's start", () => {
    const start = point("meteora_damm_v2", "2026-10-19T14:00:00Z");
    expect(
      tvlStart(
        [start, point("meteora_damm_v2", "2026-10-19T15:00:00Z")],
        "2026-10-19T14:00:00Z",
        "meteora_damm_v2",
      ),
    ).toBe(start);
  });

  // Mutation: the first point of the protocol taken, the variation starts after the floor.
  it("is null when the TVL was first measured after the window's start", () => {
    expect(
      tvlStart(
        [point("meteora_damm_v2", "2026-10-19T15:00:00Z")],
        "2026-10-19T14:00:00Z",
        "meteora_damm_v2",
      ),
    ).toBeNull();
  });

  // Mutation: the protocol filter dropped, another protocol's start is borrowed.
  it("never takes another protocol's point", () => {
    expect(
      tvlStart(
        [point("meteora_dlmm", "2026-10-19T14:00:00Z")],
        "2026-10-19T14:00:00Z",
        "meteora_damm_v2",
      ),
    ).toBeNull();
  });
});
