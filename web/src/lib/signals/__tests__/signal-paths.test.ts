import { describe, expect, it } from "vitest";

import { signalsPath } from "../signal-paths";

// Mutation: a protocol's collection read without it — tab 06 would list every protocol's signals.
describe("signal paths", () => {
  it("reads every protocol's signals without a protocol", () => {
    expect(signalsPath()).toBe("/api/signals");
  });

  it("reads one protocol's signals under its scope", () => {
    expect(signalsPath("meteora_damm_v2")).toBe("/api/protocols/meteora_damm_v2/signals");
  });
});
