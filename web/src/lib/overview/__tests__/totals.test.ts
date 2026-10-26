import { describe, expect, it } from "vitest";

import {
  changeRatio,
  discoveredTvl,
  rotation,
  shareOf,
  sumUsd,
  swapsPerSecond,
  tvlChange,
} from "../totals";

describe("sumUsd", () => {
  it("adds the protocols' amounts", () => {
    expect(sumUsd(["1.5", "2"])).toBe(3.5);
  });

  // Mutation: a null skipped publishes one protocol's amount as the total.
  it("is unknown when one term is", () => {
    expect(sumUsd(["1.5", null])).toBeNull();
  });

  it("is unknown on no row at all, not zero", () => {
    expect(sumUsd([])).toBeNull();
  });
});

describe("changeRatio", () => {
  it("is the relative change", () => {
    expect(changeRatio(110, 100)).toBeCloseTo(0.1);
  });

  // Mutation: the zero guard dropped renders "+∞ %".
  it("is unknown on a zero base", () => {
    expect(changeRatio(10, 0)).toBeNull();
  });
});

describe("shareOf", () => {
  it("is the part over the whole", () => {
    expect(shareOf(18, 100)).toBeCloseTo(0.18);
  });

  // Mutation: the zero guard dropped renders a fee rate of "∞ %" on a period without volume.
  it("is unknown on a zero whole, or an unknown term", () => {
    expect(shareOf(1, 0)).toBeNull();
    expect(shareOf(null, 10)).toBeNull();
    expect(shareOf(1, null)).toBeNull();
  });
});

describe("tvlChange", () => {
  // Mutation: the discovered part left in renders +10 %, a discovery taken for a deposit.
  it("leaves out the pools discovered in the period", () => {
    expect(tvlChange(110, 100, 8)).toBeCloseTo(0.02);
  });

  // Mutation: an unknown part read as 0 renders the raw change.
  it("is unknown when the discovered part is", () => {
    expect(tvlChange(110, 100, null)).toBeNull();
  });
});

describe("discoveredTvl", () => {
  const AT = "2026-10-25T03:00:00Z";
  const tvl = [
    { protocol: "meteora_damm_v2" as const, at: AT },
    { protocol: "meteora_dlmm" as const, at: AT },
  ];
  const part = (protocol: (typeof tvl)[number]["protocol"], at: string, tvlUsd: string) => ({
    protocol,
    at,
    tvlUsd,
  });

  it("adds the protocols' parts", () => {
    const parts = [part("meteora_damm_v2", AT, "5"), part("meteora_dlmm", AT, "2")];
    expect(discoveredTvl(tvl, parts)).toBe(7);
  });

  // Mutation: a protocol without a part skipped publishes a part that misses it.
  it("is unknown when a protocol has no part", () => {
    expect(discoveredTvl(tvl, [part("meteora_damm_v2", AT, "5")])).toBeNull();
  });

  // Mutation: the reading's instant not compared — a part measured an hour before the TVL
  // splits a TVL it was not measured with.
  it("is unknown when a part was measured at another reading than the TVL's", () => {
    const before = "2026-10-25T02:00:00Z";
    expect(
      discoveredTvl(tvl, [part("meteora_damm_v2", before, "5"), part("meteora_dlmm", AT, "2")]),
    ).toBeNull();
  });
});

describe("rotation", () => {
  // Mutation: TVL over volume renders × 2 for a liquidity that turned half over.
  it("is the volume over the TVL", () => {
    expect(rotation(50, 100)).toBe(0.5);
  });

  // Mutation: the zero guard dropped renders "× ∞".
  it("is unknown on a zero TVL", () => {
    expect(rotation(50, 0)).toBeNull();
  });

  it("is unknown when the volume is", () => {
    expect(rotation(null, 100)).toBeNull();
  });
});

describe("swapsPerSecond", () => {
  // Mutation: dividing by hours, or by the 24h window on 7d, misstates the rate.
  it("spreads the swaps over the period's seconds", () => {
    expect(swapsPerSecond(86_400 * 39, "24h")).toBe(39);
    expect(swapsPerSecond(604_800 * 39, "7d")).toBe(39);
  });
});
