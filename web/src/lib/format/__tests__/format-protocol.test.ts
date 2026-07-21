import { describe, expect, it } from "vitest";

import {
  formatProtocolLabel,
  formatProtocolShortLabel,
  protocolPlatform,
} from "../format-protocol";

describe("formatProtocolShortLabel", () => {
  it("drops the Meteora prefix, keeping the product name", () => {
    expect(formatProtocolShortLabel("meteora_damm_v2")).toBe("DAMM v2");
    expect(formatProtocolShortLabel("meteora_damm_v1")).toBe("DAMM v1");
    expect(formatProtocolShortLabel("meteora_dlmm")).toBe("DLMM");
    expect(formatProtocolShortLabel("meteora_stake2earn")).toBe("Stake2Earn");
  });

  it("falls back to Unknown for an unmapped protocol", () => {
    expect(formatProtocolShortLabel("raydium_clmm")).toBe("Unknown");
  });

  it("stays consistent with the full label (same known set)", () => {
    for (const p of ["meteora_damm_v2", "meteora_dlmm"]) {
      expect(formatProtocolLabel(p)).toContain(formatProtocolShortLabel(p));
    }
  });
});

describe("protocolPlatform", () => {
  it("maps every Meteora protocol to the meteora platform", () => {
    expect(protocolPlatform("meteora_damm_v2")).toBe("meteora");
    expect(protocolPlatform("meteora_dlmm")).toBe("meteora");
  });

  it("returns null for a platform we have no icon for", () => {
    expect(protocolPlatform("raydium_clmm")).toBeNull();
    expect(protocolPlatform("unknown")).toBeNull();
  });
});
