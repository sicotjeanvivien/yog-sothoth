import { describe, expect, it } from "vitest";

import type { SignalResponse } from "@/lib/api/schema/signal";
import { mergeSignals } from "../merge-signals";

function signal(id: number, triggeredAt: string): SignalResponse {
  return {
    id,
    detector: "flow_imbalance",
    protocol: "meteora_damm_v2",
    poolAddress: "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1",
    severity: "warning",
    value: "0.75",
    threshold: "0.6",
    message: null,
    triggeredAt,
  };
}

describe("mergeSignals", () => {
  it("prepends a newer signal in display order", () => {
    const current = [signal(2, "2026-07-02T10:00:02Z"), signal(1, "2026-07-02T10:00:01Z")];
    const merged = mergeSignals(current, [signal(3, "2026-07-02T10:00:03Z")]);
    expect(merged.map((s) => s.id)).toEqual([3, 2, 1]);
  });

  it("dedups by id when the refill overlaps the live tail", () => {
    // Post-reconnect: the refetched page contains signals the stream
    // already delivered — the union must not duplicate them.
    const current = [signal(3, "2026-07-02T10:00:03Z"), signal(2, "2026-07-02T10:00:02Z")];
    const refill = [
      signal(4, "2026-07-02T10:00:04Z"),
      signal(3, "2026-07-02T10:00:03Z"),
      signal(1, "2026-07-02T10:00:01Z"),
    ];
    const merged = mergeSignals(current, refill);
    expect(merged.map((s) => s.id)).toEqual([4, 3, 2, 1]);
  });

  it("tie-breaks equal timestamps on id desc", () => {
    const at = "2026-07-02T10:00:00Z";
    const merged = mergeSignals([signal(1, at)], [signal(2, at)]);
    expect(merged.map((s) => s.id)).toEqual([2, 1]);
  });

  it("caps the feed length, dropping the oldest", () => {
    const current = [signal(3, "2026-07-02T10:00:03Z"), signal(2, "2026-07-02T10:00:02Z")];
    const merged = mergeSignals(current, [signal(4, "2026-07-02T10:00:04Z")], 2);
    expect(merged.map((s) => s.id)).toEqual([4, 3]);
  });
});
