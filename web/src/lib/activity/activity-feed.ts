/**
 * The rows the live activity table keeps: the latest first, in their order of arrival, bounded.
 * ⚠️ Nothing is replayed after a reopening: a gap row says so, rather than a table that looks whole.
 */

import type { ActivityRow } from "./activity-row";

export const ACTIVITY_CAPACITY = 50;

export type FeedRow =
  | { type: "event"; key: string; row: ActivityRow }
  | { type: "gap"; key: string; at: string };

export function addEvent(rows: readonly FeedRow[], row: ActivityRow): readonly FeedRow[] {
  if (rows.some((r) => r.key === row.key)) {
    return rows;
  }
  return [{ type: "event", key: row.key, row } as const, ...rows].slice(0, ACTIVITY_CAPACITY);
}

/** A gap at `at`, only over rows already shown and never twice in a row. */
export function markGap(rows: readonly FeedRow[], at: string): readonly FeedRow[] {
  const top = rows[0];
  if (top === undefined || top.type === "gap") {
    return rows;
  }
  return [{ type: "gap", key: `gap:${at}`, at } as const, ...rows].slice(0, ACTIVITY_CAPACITY);
}
