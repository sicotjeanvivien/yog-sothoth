import { describe, expect, it } from "vitest";

import { seriesOf } from "@/lib/overview/__tests__/series";
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

describe("splitSlotsOf", () => {
  // Mutation: the response's step not read — the last hour drawn on 24 hourly slots.
  it("draws the last hour on its twelve tranches", () => {
    const at = new Date(Date.parse(AS_OF) - 5 * 60_000).toISOString();
    const slots = splitSlotsOf(seriesOf("5m", AS_OF, [bar(at, "30", "20", "10")]));

    expect(slots).toHaveLength(12);
    expect(slots.at(-1)).toMatchObject({ total: 30, inRuns: 20, outside: 10 });
  });

  // Mutation: a part read from another's field — the volume outside the runs drawn in them.
  it("carries each hour's split as the response gives it", () => {
    const slots = splitSlotsOf(seriesOf("1h", AS_OF, [bar(hour(22), "100", "80", "20"), bar(hour(23), "50", "0", "50")]));

    expect(slots.slice(22).map((s) => [s.total, s.inRuns, s.outside])).toEqual([
      [100, 80, 20],
      [50, 0, 50],
    ]);
  });

  // Mutation: an unsplit bar read as 0 — a bar the runs do not cover drawn as split.
  it("keeps the total of a bar the runs do not split", () => {
    const slices = splitSlotsOf(seriesOf("6h", AS_OF, [bar("2026-10-24T21:00:00Z", "60", null, null)]));

    expect(slices.at(-1)).toMatchObject({ total: 60, inRuns: null, outside: null });
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
