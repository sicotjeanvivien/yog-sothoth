import { describe, expect, it } from "vitest";

import type { SignalResponse } from "@/lib/api/schema/zod.gen";
import { withGaps } from "../signal-gaps";

function signal(id: number, triggeredAt: string): SignalResponse {
  return { id, triggeredAt } as SignalResponse;
}

const shape = (rows: ReturnType<typeof withGaps>) =>
  rows.map((row) => (row.type === "gap" ? `gap ${row.at}` : `signal ${row.signal.id}`));

describe("withGaps", () => {
  // Mutation: gaps dropped — a feed that missed signals reads as complete.
  it("puts a gap at its instant, between the signals before and after it", () => {
    const signals = [signal(3, "2026-11-06T10:05:00Z"), signal(2, "2026-11-06T10:01:00Z")];

    expect(shape(withGaps(signals, ["2026-11-06T10:03:00Z"]))).toEqual([
      "signal 3",
      "gap 2026-11-06T10:03:00Z",
      "signal 2",
    ]);
  });

  // Mutation: a gap shown over nothing — a fresh page would open on "signals may have been missed".
  it("leaves out a gap no older signal is under, and keeps one above the newest", () => {
    const signals = [signal(3, "2026-11-06T10:05:00Z")];

    expect(shape(withGaps(signals, ["2026-11-06T10:06:00Z", "2026-11-06T10:04:00Z"]))).toEqual([
      "gap 2026-11-06T10:06:00Z",
      "signal 3",
    ]);
    expect(shape(withGaps([], ["2026-11-06T10:06:00Z"]))).toEqual([]);
  });

  // Mutation: each reopening its own row — two drops in a row stack two notices.
  it("keeps the latest of the gaps between the same two signals", () => {
    const signals = [signal(3, "2026-11-06T10:09:00Z"), signal(2, "2026-11-06T10:01:00Z")];

    expect(shape(withGaps(signals, ["2026-11-06T10:03:00Z", "2026-11-06T10:07:00Z"]))).toEqual([
      "signal 3",
      "gap 2026-11-06T10:07:00Z",
      "signal 2",
    ]);
  });
});
