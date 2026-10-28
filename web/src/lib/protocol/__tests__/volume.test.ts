import { describe, expect, it } from "vitest";

import {
  durationOf,
  outsideRuns,
  runsMeasuredSince,
  splitSlotsOf,
  splitTotalsOf,
  type SplitSlot,
} from "../volume";

const bar = (from: string, volumeUsd: string | null) => ({ from, volumeUsd });

const AS_OF = "2026-10-25T03:00:00Z";

const runsOf = (items: ReturnType<typeof bar>[], coveredSince = "2026-10-01T00:00:00Z", asOf = AS_OF) => ({
  asOf,
  coveredSince,
  items,
});

const hour = (h: number) => new Date(Date.parse(AS_OF) - (24 - h) * 3_600_000).toISOString();

describe("splitSlotsOf", () => {
  // Mutation: an hour without a run left unsplit — an hour all outside the runs drawn neutral.
  it("splits each bar by its runs, an hour without one wholly outside them", () => {
    const slots = splitSlotsOf(
      "24h",
      AS_OF,
      [bar(hour(22), "100"), bar(hour(23), "50")],
      runsOf([bar(hour(22), "80"), bar(hour(23), "0")]),
    );

    expect(slots.slice(22).map((s) => [s.total, s.inRuns])).toEqual([
      [100, 80],
      [50, 0],
    ]);
  });

  const hoursFrom = (from: string, values: string[]) =>
    values.map((v, h) => bar(new Date(Date.parse(from) + h * 3_600_000).toISOString(), v));

  // Mutation: a slice of runs missing an hour read as none in runs — its whole volume drawn
  // outside them; or the runs' hours not summed — a slice worth one hour of runs.
  it("splits a 7 days' slice by its runs' hours, and leaves it unsplit when one is missing", () => {
    const slices = splitSlotsOf(
      "7d",
      AS_OF,
      [...hoursFrom("2026-10-24T15:00:00Z", Array(12).fill("10"))],
      runsOf([
        ...hoursFrom("2026-10-24T15:00:00Z", ["2", "0", "3", "0", "0", "0"]),
        ...hoursFrom("2026-10-24T21:00:00Z", ["10", "10", "10", "10", "10"]),
      ]),
    );

    expect(slices.slice(26).map((s) => [s.total, s.inRuns])).toEqual([
      [60, 5],
      [60, null],
    ]);
  });

  // Mutation: a bar counted as covered from its end, or the coverage dropped — a day the runs
  // only partly measured split as whole.
  it("leaves unsplit a bar that starts before the runs are measured", () => {
    const days = splitSlotsOf(
      "30d",
      AS_OF,
      [bar("2026-10-18T03:00:00Z", "700"), bar("2026-10-19T03:00:00Z", "300")],
      runsOf([bar("2026-10-19T03:00:00Z", "200")], "2026-10-18T04:00:00Z"),
    );

    expect(days.slice(23, 25).map((s) => [s.total, s.inRuns])).toEqual([
      [700, null],
      [300, 200],
    ]);
  });

  // Mutation: the runs' end not checked — read an hour later, their days start an hour later,
  // and every day reads as none in runs.
  it("leaves every bar unsplit when the runs end at another hour", () => {
    const later = "2026-10-25T04:00:00Z";
    const days = splitSlotsOf(
      "30d",
      AS_OF,
      [bar("2026-10-24T03:00:00Z", "300")],
      runsOf([bar("2026-10-24T04:00:00Z", "200")], "2026-10-01T00:00:00Z", later),
    );

    expect(days.map((s) => s.inRuns)).toEqual(Array(30).fill(null));
  });
});

const slot = (total: number | null, inRuns: number | null): SplitSlot => ({
  from: new Date(0),
  to: new Date(0),
  total,
  inRuns,
});

describe("splitTotalsOf", () => {
  // Mutation: an unsplit bar summed as outside the runs — the share in runs understated.
  it("sums the split bars apart from the unsplit ones", () => {
    const totals = splitTotalsOf([slot(100, 90), slot(60, 0), slot(500, null), slot(null, null)]);

    expect(totals).toEqual({ inRuns: 90, outside: 70, share: 90 / 160, unsplit: 500 });
  });

  it("has no share without split volume", () => {
    expect(splitTotalsOf([slot(500, null)]).share).toBeNull();
  });
});

describe("outsideRuns", () => {
  it("is never under zero", () => {
    expect(outsideRuns(slot(10, 10.000001))).toBe(0);
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

describe("runsMeasuredSince", () => {
  // Mutation: `coveredSince` taken as is — runs never measured said measured since the last hour.
  it("is nothing while the coverage starts at the end", () => {
    expect(runsMeasuredSince({ asOf: AS_OF, coveredSince: AS_OF })).toBeNull();
    expect(runsMeasuredSince({ asOf: null, coveredSince: null })).toBeNull();
    expect(runsMeasuredSince({ asOf: AS_OF, coveredSince: "2026-10-18T04:00:00Z" })).toBe(
      "2026-10-18T04:00:00Z",
    );
  });
});
