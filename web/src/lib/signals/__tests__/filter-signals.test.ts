import { describe, expect, it } from "vitest";

import type { SeverityResponse, SignalResponse } from "@/lib/api/schema/zod.gen";
import { filterSignals } from "../filter-signals";
import type { SignalFeedRow } from "../merge-signals";

function signal(
  id: number,
  severity: SeverityResponse,
  detector: string,
): SignalResponse {
  return {
    id,
    detector,
    protocol: "meteora_damm_v2",
    poolAddress: "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1",
    tokenA: {
      mint: null,
      symbol: null,
      name: null,
      decimals: null,
      logoUri: null,
    },
    tokenB: {
      mint: null,
      symbol: null,
      name: null,
      decimals: null,
      logoUri: null,
    },
    severity,
    value: "0.75",
    threshold: "0.6",
    valueUnit: "raw",
    leansToward: null,
    message: null,
    triggeredAt: "2026-07-06T10:00:00Z",
  };
}

function row(s: SignalResponse): SignalFeedRow {
  return { type: "signal", key: `signal:${s.id}`, signal: s };
}

const GAP: SignalFeedRow = { type: "gap", key: "gap:2026-07-06T10:00:00Z", at: "2026-07-06T10:00:00Z" };

const FEED = [
  row(signal(1, "critical", "tvl_drain")),
  row(signal(2, "warning", "tvl_drain")),
  GAP,
  row(signal(3, "warning", "flow_imbalance")),
  row(signal(4, "info", "some_future_detector")),
];

function idsOf(rows: readonly SignalFeedRow[]): (number | "gap")[] {
  return rows.map((r) => (r.type === "signal" ? r.signal.id : "gap"));
}

const none = new Set<never>();

describe("filterSignals", () => {
  it("empty selections filter nothing (the default hides nothing)", () => {
    expect(filterSignals(FEED, none, none)).toBe(FEED);
  });

  it("filters by severity, OR within the dimension", () => {
    const out = filterSignals(FEED, new Set(["critical", "info"]), none);
    expect(idsOf(out)).toEqual([1, "gap", 4]);
  });

  it("filters by detector", () => {
    const out = filterSignals(FEED, none, new Set(["flow_imbalance"]));
    expect(idsOf(out)).toEqual(["gap", 3]);
  });

  it("ANDs across dimensions", () => {
    const out = filterSignals(
      FEED,
      new Set(["warning"]),
      new Set(["tvl_drain"]),
    );
    expect(idsOf(out)).toEqual([2, "gap"]);
  });

  it("unknown detectors are filterable like any other", () => {
    const out = filterSignals(FEED, none, new Set(["some_future_detector"]));
    expect(idsOf(out)).toEqual(["gap", 4]);
  });

  it("can leave no signal, the gap row alone", () => {
    const out = filterSignals(
      FEED,
      new Set(["critical"]),
      new Set(["flow_imbalance"]),
    );
    expect(idsOf(out)).toEqual(["gap"]);
  });

  // Mutation: the gap row filtered like a signal.
  it("keeps the gap row under every filter", () => {
    expect(filterSignals([GAP], new Set(["critical"]), new Set(["flow_imbalance"]))).toEqual([GAP]);
  });
});
