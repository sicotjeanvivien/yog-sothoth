import { describe, expect, it } from "vitest";

import { scopedPath } from "../scope";

describe("scopedPath", () => {
  // Mutation: the protocol's route read on the Overview, or the root on a protocol's page.
  it("reads every protocol at the root and one protocol under its own path", () => {
    expect(scopedPath("all", "swaps/totals")).toBe("/api/swaps/totals");
    expect(scopedPath("meteora_damm_v2", "swaps/totals")).toBe(
      "/api/protocols/meteora_damm_v2/swaps/totals",
    );
  });
});
