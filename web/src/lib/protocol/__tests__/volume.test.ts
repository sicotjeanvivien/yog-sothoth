import { describe, expect, it } from "vitest";

import {
  durationOf,
  splitSinceOf,
  splitSlotsOf,
  splitTotalsOf,
  type SplitSlot,
} from "../volume";

const AS_OF = "2026-10-25T03:00:00Z";

const bar = (from: string, volume: string | null, inRuns: string | null, outside: string | null) => ({
  from,
  volumeUsd: volume,
  inRunsUsd: inRuns,
  outsideRunsUsd: outside,
});

const hour = (h: number) => new Date(Date.parse(AS_OF) - (24 - h) * 3_600_000).toISOString();

const hoursFrom = (from: string, parts: [string, string | null, string | null][]) =>
  parts.map(([v, i, o], h) => bar(new Date(Date.parse(from) + h * 3_600_000).toISOString(), v, i, o));

describe("splitSlotsOf", () => {
  // Mutation: a part read from another's field — the volume outside the runs drawn in them.
  it("carries each hour's split as the response gives it", () => {
    const slots = splitSlotsOf("24h", AS_OF, [bar(hour(22), "100", "80", "20"), bar(hour(23), "50", "0", "50")]);

    expect(slots.slice(22).map((s) => [s.total, s.inRuns, s.outside])).toEqual([
      [100, 80, 20],
      [50, 0, 50],
    ]);
  });

  // Mutation: an unsplit hour summed as 0 in its slice — a slice the runs half measured split as
  // whole.
  it("splits a 7 days' slice only when every hour of it is split", () => {
    const slices = splitSlotsOf("7d", AS_OF, [
      ...hoursFrom("2026-10-24T15:00:00Z", [
        ["10", "2", "8"],
        ["10", "0", "10"],
        ["10", "3", "7"],
        ["10", "0", "10"],
        ["10", "0", "10"],
        ["10", "0", "10"],
      ]),
      ...hoursFrom("2026-10-24T21:00:00Z", [
        ["10", null, null],
        ["10", "1", "9"],
        ["10", "1", "9"],
        ["10", "1", "9"],
        ["10", "1", "9"],
        ["10", "1", "9"],
      ]),
    ]);

    expect(slices.slice(26).map((s) => [s.total, s.inRuns, s.outside])).toEqual([
      [60, 5, 55],
      [60, null, null],
    ]);
  });
});

describe("splitSinceOf", () => {
  // Mutation: the first bar taken whatever its split — runs never measured said measured since
  // the window's start.
  it("is the first bar the runs split, nothing before any is", () => {
    expect(splitSinceOf([bar(hour(21), "1", null, null), bar(hour(22), "1", "0", "1")])).toBe(hour(22));
    expect(splitSinceOf([bar(hour(21), "1", null, null)])).toBeNull();
  });
});

const slot = (total: number | null, inRuns: number | null, outside: number | null): SplitSlot => ({
  from: new Date(0),
  to: new Date(0),
  total,
  inRuns,
  outside,
});

describe("splitTotalsOf", () => {
  // Mutation: an unsplit bar summed as outside the runs — the share in runs understated.
  it("sums the split bars apart from the unsplit ones", () => {
    const totals = splitTotalsOf([slot(100, 90, 10), slot(60, 0, 60), slot(500, null, null), slot(null, null, null)]);

    expect(totals).toEqual({ inRuns: 90, outside: 70, share: 90 / 160, unsplit: 500 });
  });

  it("has no share without split volume", () => {
    expect(splitTotalsOf([slot(500, null, null)]).share).toBeNull();
  });
});

describe("durationOf", () => {
  // Mutation: the hours dropped, or the minutes not padded.
  it("says minutes under an hour, hours and minutes beyond", () => {
    expect(durationOf(300)).toBe("5 min");
    expect(durationOf(3_300)).toBe("55 min");
    expect(durationOf(3_900)).toBe("1 h 05");
    expect(durationOf(9_000)).toBe("2 h 30");
  });
});
