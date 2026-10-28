import { describe, expect, it } from "vitest";

import { axisOf, daysAgoOf, roundTicks, slotsOf } from "../chart";

describe("roundTicks", () => {
  it("keeps the largest d × 10ⁿ under the maximum, and its half", () => {
    expect(roundTicks(12_400)).toEqual([10_000, 5_000]);
  });

  // Mutation: the 60 % rule dropped leaves 1 000 under a bar of 1 900.
  it("falls back to a half step when d × 10ⁿ leaves the top bar too high", () => {
    expect(roundTicks(1_900)).toEqual([1_500, 750]);
  });

  it("draws no mark on an empty series", () => {
    expect(roundTicks(0)).toEqual([]);
  });
});

describe("slotsOf", () => {
  const asOf = "2026-10-20T14:00:00Z";

  it("lays 24 hourly slots ending at asOf, summed across protocols", () => {
    const slots = slotsOf("24h", asOf, [
      { from: "2026-10-20T13:00:00Z", value: "10" },
      { from: "2026-10-20T13:00:00Z", value: "5.5" },
    ]);
    expect(slots).toHaveLength(24);
    expect(slots[0]?.from).toEqual(new Date("2026-10-19T14:00:00Z"));
    expect(slots[23]).toEqual({
      from: new Date("2026-10-20T13:00:00Z"),
      to: new Date(asOf),
      value: 15.5,
    });
  });

  // Mutation: a bar placed by its index, not its `from`, closes the hole on the left.
  it("leaves the slot of a missing bar empty", () => {
    const slots = slotsOf("30d", asOf, [
      { from: "2026-10-19T14:00:00Z", value: "7" },
    ]);
    expect(slots).toHaveLength(30);
    expect(slots[0]?.value).toBeNull();
    expect(slots[29]?.value).toBe(7);
  });

  // Mutation: the grid not checked — a day read at another `asOf` counted in the slot it overlaps.
  it("puts a bar off the slots' grid in none of them", () => {
    const slots = slotsOf("30d", asOf, [{ from: "2026-10-19T15:00:00Z", value: "7" }]);
    expect(slots.every((s) => s.value === null)).toBe(true);
  });

  const hoursOf = (from: string, hours: number, value: string | null = "1") =>
    Array.from({ length: hours }, (_, h) => ({
      from: new Date(Date.parse(from) + h * 3_600_000).toISOString(),
      value,
    }));

  // Mutation: 7 days cut into days, or an hour kept only where a slice starts — 7 slots, or a
  // slice worth its first hour.
  it("lays 28 slices of 6 hours ending at asOf over 7 days, each summing its hours", () => {
    const slots = slotsOf("7d", asOf, [
      ...hoursOf("2026-10-20T08:00:00Z", 6, "10"),
      ...hoursOf("2026-10-20T08:00:00Z", 6, "0.5"),
    ]);
    expect(slots).toHaveLength(28);
    expect(slots[0]?.from).toEqual(new Date("2026-10-13T14:00:00Z"));
    expect(slots[27]).toEqual({
      from: new Date("2026-10-20T08:00:00Z"),
      to: new Date(asOf),
      value: 63,
    });
  });

  // Mutation: a slice missing an hour emptied — a coverage rule the API's days do not apply.
  it("sums the hours a slice holds, without judging whether it is whole", () => {
    const slots = slotsOf("7d", asOf, hoursOf("2026-10-20T09:00:00Z", 5));
    expect(slots[27]?.value).toBe(5);
  });

  // Mutation: an unvalued hour emptying its slice — unlike the SUM of the API's days.
  it("sums the valued hours of a slice, an unvalued one left out", () => {
    const slots = slotsOf("7d", asOf, [
      ...hoursOf("2026-10-20T08:00:00Z", 5, "2"),
      ...hoursOf("2026-10-20T13:00:00Z", 1, null),
    ]);
    expect(slots[27]?.value).toBe(10);
  });
});

describe("daysAgoOf", () => {
  // Mutation: the slot's index taken for its day — the 7 days' slices six days apart each.
  it("counts the 24-hour slices before asOf a slot falls in", () => {
    expect([0, 3, 4, 23, 24, 27].map((i) => daysAgoOf("7d", 28, i))).toEqual([6, 6, 5, 1, 0, 0]);
    expect([0, 29].map((i) => daysAgoOf("30d", 30, i))).toEqual([29, 0]);
  });
});

describe("axisOf", () => {
  const hours = slotsOf("24h", "2026-10-24T12:00:00Z", []);

  // Mutation: the last slot left to the step — the axis would end hours before `asOf`.
  it("labels every sixth hour, and always the last one in UTC", () => {
    expect(axisOf("24h", hours, String)).toEqual(["12:00", "18:00", "00:00", "06:00", "11:00 UTC"]);
  });

  // Mutation: the 30 days' rule over the 7 days' slices — today named twice.
  it("names each of the 7 days once, by how long ago it was", () => {
    const slices = slotsOf("7d", "2026-10-24T12:00:00Z", []);
    expect(axisOf("7d", slices, (d) => `J-${d}`)).toEqual(["J-6", "J-5", "J-4", "J-3", "J-2", "J-1", "J-0"]);
  });
});
