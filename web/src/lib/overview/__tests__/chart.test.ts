import { describe, expect, it } from "vitest";

import { axisOf, roundTicks, slotsOf } from "../chart";

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
    const slots = slotsOf("7d", asOf, [
      { from: "2026-10-19T14:00:00Z", value: "7" },
    ]);
    expect(slots).toHaveLength(7);
    expect(slots[0]?.value).toBeNull();
    expect(slots[6]?.value).toBe(7);
  });
});

describe("axisOf", () => {
  const hours = slotsOf("24h", "2026-10-24T12:00:00Z", []);

  // Mutation: the last slot left to the step — the axis would end hours before `asOf`.
  it("labels every sixth hour, and always the last one in UTC", () => {
    expect(axisOf("24h", hours, String)).toEqual(["12:00", "18:00", "00:00", "06:00", "11:00 UTC"]);
  });

  it("names the days by how long ago they were", () => {
    const days = slotsOf("7d", "2026-10-24T12:00:00Z", []);
    expect(axisOf("7d", days, (d) => `J-${d}`)).toEqual(["J-6", "J-5", "J-4", "J-3", "J-2", "J-1", "J-0"]);
  });
});
