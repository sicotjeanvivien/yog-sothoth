import { describe, expect, it } from "vitest";

import { liveSignalsPath, signalsPath } from "../signal-paths";

// Mutation: a protocol's paths built without it — tab 06 would show every protocol's signals.
describe("signal paths", () => {
  it("reads every protocol's signals without a protocol", () => {
    expect(signalsPath()).toBe("/api/signals");
    expect(liveSignalsPath()).toBe("/api/live?topics=signals");
  });

  it("reads one protocol's signals, page and live stream alike", () => {
    expect(signalsPath("meteora_damm_v2")).toBe("/api/protocols/meteora_damm_v2/signals");
    expect(liveSignalsPath("meteora_damm_v2")).toBe(
      "/api/live/protocols/meteora_damm_v2?topics=signals",
    );
  });
});
