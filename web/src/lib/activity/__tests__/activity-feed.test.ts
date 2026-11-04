import { describe, expect, it } from "vitest";

import { ACTIVITY_CAPACITY, addEvent, markGap, type FeedRow } from "../activity-feed";
import type { ActivityRow } from "../activity-row";

function row(n: number): ActivityRow {
  return { key: `k${n}`, at: "2026-11-04T21:14:05Z", protocol: "meteora_damm_v2", kind: "pool_creation", pool: "p", detail: [] };
}

const keys = (rows: readonly FeedRow[]) => rows.map((r) => r.key);

describe("activity feed", () => {
  // Mutation: new rows appended — the latest event would sink under the older ones.
  it("puts each new event on top", () => {
    const rows = addEvent(addEvent([], row(1)), row(2));
    expect(keys(rows)).toEqual(["k2", "k1"]);
  });

  // Mutation: the bound dropped — the table grows for as long as the tab stays open.
  it("keeps the latest rows only, and an event once", () => {
    let rows: readonly FeedRow[] = [];
    for (let n = 0; n < ACTIVITY_CAPACITY + 5; n += 1) {
      rows = addEvent(rows, row(n));
    }
    expect(rows).toHaveLength(ACTIVITY_CAPACITY);
    expect(rows[0]?.key).toBe(`k${ACTIVITY_CAPACITY + 4}`);
    expect(addEvent(rows, row(ACTIVITY_CAPACITY + 4))).toBe(rows);
  });

  // Mutation: a gap on an empty table, or twice in a row — a reopening says nothing to recover.
  it("marks a gap over shown rows only, once", () => {
    expect(markGap([], "2026-11-04T21:15:00Z")).toEqual([]);
    const gapped = markGap(addEvent([], row(1)), "2026-11-04T21:15:00Z");
    expect(gapped.map((r) => r.type)).toEqual(["gap", "event"]);
    expect(markGap(gapped, "2026-11-04T21:16:00Z")).toBe(gapped);
  });
});
