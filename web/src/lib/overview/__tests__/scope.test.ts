import { describe, expect, it } from "vitest";

import { inScope } from "../scope";
import { sumUsd } from "../totals";

const rows = [
  { protocol: "meteora_damm_v2" as const, volumeUsd: "10" },
  { protocol: "meteora_dlmm" as const, volumeUsd: "5" },
];

describe("inScope", () => {
  it("keeps every protocol on the Overview", () => {
    expect(sumUsd(inScope(rows, "all").map((r) => r.volumeUsd))).toBe(15);
  });

  // Mutation: without the filter, a protocol's page publishes the total of all protocols.
  it("keeps only the page's protocol", () => {
    expect(sumUsd(inScope(rows, "meteora_damm_v2").map((r) => r.volumeUsd))).toBe(10);
  });

  it("leaves nothing to sum when the protocol has no row", () => {
    expect(inScope(rows.slice(1), "meteora_damm_v2")).toEqual([]);
  });
});
