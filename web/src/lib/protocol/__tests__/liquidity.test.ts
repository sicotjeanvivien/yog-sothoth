import { describe, expect, it } from "vitest";

import type { ProtocolResponse } from "@/lib/api/schema/zod.gen";

import { CURVE_BOX, curveOf, flowSlotsOf } from "../liquidity";

const tvl = (at: string, tvlUsd: string | null, protocol: ProtocolResponse = "meteora_damm_v2") => ({
  protocol,
  at,
  tvlUsd,
});

describe("curveOf", () => {
  // Mutation: the axis from zero — a move of 1 % on $18 M would draw a flat line.
  it("spans the axis from the lowest to the highest TVL", () => {
    const curve = curveOf(
      [tvl("2026-10-24T01:00:00Z", "17750000"), tvl("2026-10-24T02:00:00Z", "18300000")],
      "meteora_damm_v2",
    );

    expect(curve?.low).toBe(17_750_000);
    expect(curve?.high).toBe(18_300_000);
    expect(curve?.coords[0]?.y).toBeGreaterThan(curve?.coords[1]?.y ?? Infinity);
    expect(curve?.coords.at(-1)?.x).toBe(CURVE_BOX.width);
  });

  // Mutation: the other protocols kept, their TVL drawn into this one's.
  it("keeps the protocol's points only, and leaves out a point without a value", () => {
    const curve = curveOf(
      [
        tvl("2026-10-24T01:00:00Z", "10"),
        tvl("2026-10-24T01:00:00Z", "999", "meteora_dlmm"),
        tvl("2026-10-24T02:00:00Z", null),
        tvl("2026-10-24T03:00:00Z", "12"),
      ],
      "meteora_damm_v2",
    );

    expect(curve?.points.map((p) => p.value)).toEqual([10, 12]);
  });

  it("is nothing under two points", () => {
    expect(curveOf([tvl("2026-10-24T01:00:00Z", "10")], "meteora_damm_v2")).toBeNull();
  });
});

describe("flowSlotsOf", () => {
  const bar = (
    from: string,
    liquidity: { createdUsd: string; addedUsd: string; removedUsd: string } | null,
  ) => ({
    protocol: "meteora_damm_v2" as const,
    from,
    liquidity,
  });

  // Mutation: a bar before the liquidity was measured read as zero — a day without any move.
  it("keeps a bar not measured as nothing, not zero", () => {
    const slots = flowSlotsOf(
      "24h",
      "2026-10-24T12:00:00Z",
      [
        bar("2026-10-24T10:00:00Z", null),
        bar("2026-10-24T11:00:00Z", { createdUsd: "7", addedUsd: "5", removedUsd: "2" }),
      ],
      "meteora_damm_v2",
    );

    expect(slots).toHaveLength(24);
    expect(slots.at(-2)).toMatchObject({ created: null, added: null, removed: null });
    // Mutation: the creations read from the added column — a creation drawn as an addition.
    expect(slots.at(-1)).toMatchObject({ created: 7, added: 5, removed: 2 });
  });
});
