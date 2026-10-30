import { describe, expect, it } from "vitest";

import {
  previousWindowAdjoins,
  previousWindowCovered,
} from "../availability";

const FIRST_HOUR = "2026-10-13T13:00:00Z";

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

describe("previousWindowAdjoins", () => {
  const current = { from: "2026-10-20T13:05:00Z", asOf: "2026-10-20T14:05:00Z", coveredSince: null };

  // Mutation: the check dropped — a previous hour read after a tranche overlaps the current one.
  it("holds a previous window that ends where the current one starts, and no other", () => {
    expect(
      previousWindowAdjoins({ from: null, asOf: "2026-10-20T13:05:00Z", coveredSince: null }, current),
    ).toBe(true);
    expect(
      previousWindowAdjoins({ from: null, asOf: "2026-10-20T13:10:00Z", coveredSince: null }, current),
    ).toBe(false);
  });
});
