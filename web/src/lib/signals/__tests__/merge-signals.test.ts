import { describe, expect, it } from "vitest";

import type { SignalResponse } from "@/lib/api/schema/zod.gen";
import { FEED_CAP, markSignalGap, mergeSignals, type SignalFeedRow } from "../merge-signals";

function signal(id: number, triggeredAt: string): SignalResponse {
  return {
    id,
    detector: "flow_imbalance",
    protocol: "meteora_damm_v2",
    poolAddress: "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1",
    tokenA: {
      mint: "So11111111111111111111111111111111111111112",
      symbol: "SOL",
      name: "Wrapped SOL",
      decimals: 9,
      logoUri: null,
    },
    tokenB: {
      mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
      symbol: "USDC",
      name: "USD Coin",
      decimals: 6,
      logoUri: null,
    },
    severity: "warning",
    value: "0.75",
    threshold: "0.6",
    valueUnit: "signedRatio",
    leansToward: "tokenB",
    message: null,
    triggeredAt,
  };
}

function rows(...signals: SignalResponse[]): SignalFeedRow[] {
  return mergeSignals([], signals);
}

function shape(feed: readonly SignalFeedRow[]): (number | string)[] {
  return feed.map((r) => (r.type === "signal" ? r.signal.id : `gap ${r.at.slice(11, 19)}`));
}

describe("mergeSignals", () => {
  it("prepends a newer signal in display order", () => {
    const current = rows(signal(2, "2026-07-02T10:00:02Z"), signal(1, "2026-07-02T10:00:01Z"));
    const merged = mergeSignals(current, [signal(3, "2026-07-02T10:00:03Z")]);
    expect(shape(merged)).toEqual([3, 2, 1]);
  });

  // Mutation: the union keyed on anything but the signal's id.
  it("dedups by id when a signal arrives twice", () => {
    const current = rows(signal(3, "2026-07-02T10:00:03Z"), signal(2, "2026-07-02T10:00:02Z"));
    const merged = mergeSignals(current, [signal(3, "2026-07-02T10:00:03Z")]);
    expect(shape(merged)).toEqual([3, 2]);
  });

  it("tie-breaks equal timestamps on id desc", () => {
    const at = "2026-07-02T10:00:00Z";
    const merged = mergeSignals(rows(signal(1, at)), [signal(2, at)]);
    expect(shape(merged)).toEqual([2, 1]);
  });

  it("caps the feed length, dropping the oldest", () => {
    const current = rows(signal(3, "2026-07-02T10:00:03Z"), signal(2, "2026-07-02T10:00:02Z"));
    const merged = mergeSignals(current, [signal(4, "2026-07-02T10:00:04Z")], 2);
    expect(shape(merged)).toEqual([4, 3]);
  });

  // Mutation: the gap rows dropped by the merge.
  it("keeps the gap rows, sorted with the signals by time", () => {
    const feed = markSignalGap(rows(signal(1, "2026-07-02T10:00:01Z")), "2026-07-02T10:00:05Z");
    const merged = mergeSignals(feed, [signal(2, "2026-07-02T10:00:09Z")]);
    expect(shape(merged)).toEqual([2, "gap 10:00:05", 1]);
  });
});

describe("markSignalGap", () => {
  // Mutation: the gap pinned on top, in its order of arrival.
  it("ranks a signal triggered before the reopening below the gap, though it arrives after", () => {
    const feed = markSignalGap(rows(signal(1, "2026-07-02T10:00:01Z")), "2026-07-02T10:00:05Z");
    const merged = mergeSignals(feed, [
      signal(2, "2026-07-02T10:00:30Z"),
      signal(3, "2026-07-02T10:00:03Z"),
    ]);
    expect(shape(merged)).toEqual([2, "gap 10:00:05", 3, 1]);
  });

  // Mutation: the tie broken the other way.
  it("ranks a signal triggered at the very instant of the reopening above the gap", () => {
    const at = "2026-07-02T10:00:05Z";
    const feed = markSignalGap(rows(signal(1, "2026-07-02T10:00:01Z")), at);
    expect(shape(mergeSignals(feed, [signal(2, at)]))).toEqual([2, "gap 10:00:05", 1]);
  });

  // Mutation: the empty-feed guard removed.
  it("marks nothing on an empty feed", () => {
    expect(markSignalGap([], "2026-07-02T10:00:05Z")).toEqual([]);
  });

  // Mutation: the guard against two gaps with no signal between them removed.
  it("marks nothing when no signal came since the last gap", () => {
    const once = markSignalGap(rows(signal(1, "2026-07-02T10:00:01Z")), "2026-07-02T10:00:05Z");
    expect(shape(markSignalGap(once, "2026-07-02T10:00:09Z"))).toEqual(["gap 10:00:05", 1]);
  });

  it("marks a second gap once a signal came between", () => {
    const once = markSignalGap(rows(signal(1, "2026-07-02T10:00:01Z")), "2026-07-02T10:00:05Z");
    const fed = mergeSignals(once, [signal(2, "2026-07-02T10:00:07Z")]);
    expect(shape(markSignalGap(fed, "2026-07-02T10:00:09Z"))).toEqual([
      "gap 10:00:09",
      2,
      "gap 10:00:05",
      1,
    ]);
  });

  it("keeps the feed within its cap", () => {
    const full = rows(
      ...Array.from({ length: FEED_CAP }, (_, i) =>
        signal(i + 1, new Date(Date.UTC(2026, 6, 2, 10, 0, 0, i)).toISOString()),
      ),
    );
    const marked = markSignalGap(full, "2026-07-02T11:00:00Z");
    expect(marked).toHaveLength(FEED_CAP);
    expect(marked[0]?.type).toBe("gap");
  });
});
