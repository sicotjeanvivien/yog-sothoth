import { describe, expect, it } from "vitest";

import type { Severity, SignalResponse } from "@/lib/api/schema/signal";
import { filterSignals } from "../filter-signals";

function signal(
  id: number,
  severity: Severity,
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
      decimals: 0,
      logoUri: null,
      price: null,
    },
    tokenB: {
      mint: null,
      symbol: null,
      name: null,
      decimals: 0,
      logoUri: null,
      price: null,
    },
    severity,
    value: "0.75",
    threshold: "0.6",
    message: null,
    triggeredAt: "2026-07-06T10:00:00Z",
  };
}

const FEED = [
  signal(1, "critical", "price_oracle_deviation"),
  signal(2, "warning", "price_oracle_deviation"),
  signal(3, "warning", "flow_imbalance"),
  signal(4, "info", "some_future_detector"),
];

const none = new Set<never>();

describe("filterSignals", () => {
  it("empty selections filter nothing (the default hides nothing)", () => {
    expect(filterSignals(FEED, none, none)).toBe(FEED);
  });

  it("filters by severity, OR within the dimension", () => {
    const out = filterSignals(FEED, new Set(["critical", "info"]), none);
    expect(out.map((s) => s.id)).toEqual([1, 4]);
  });

  it("filters by detector", () => {
    const out = filterSignals(FEED, none, new Set(["flow_imbalance"]));
    expect(out.map((s) => s.id)).toEqual([3]);
  });

  it("ANDs across dimensions", () => {
    const out = filterSignals(
      FEED,
      new Set(["warning"]),
      new Set(["price_oracle_deviation"]),
    );
    expect(out.map((s) => s.id)).toEqual([2]);
  });

  it("unknown detectors are filterable like any other", () => {
    const out = filterSignals(FEED, none, new Set(["some_future_detector"]));
    expect(out.map((s) => s.id)).toEqual([4]);
  });

  it("can produce an empty result", () => {
    const out = filterSignals(
      FEED,
      new Set(["critical"]),
      new Set(["flow_imbalance"]),
    );
    expect(out).toEqual([]);
  });
});
