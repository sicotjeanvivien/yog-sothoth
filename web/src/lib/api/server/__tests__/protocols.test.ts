import { describe, expect, it } from "vitest";

import { servesTvlSeries } from "../protocols";

describe("servesTvlSeries", () => {
  // Mutation: the guard read off the contract — the last hour asked of a route that refuses it.
  it("serves the windows the contract lists, at the root and under a protocol", () => {
    for (const scope of ["all", "meteora_damm_v2"] as const) {
      expect(servesTvlSeries(scope, "24h")).toBe(true);
      expect(servesTvlSeries(scope, "30d")).toBe(true);
      expect(servesTvlSeries(scope, "1h")).toBe(false);
    }
  });
});
