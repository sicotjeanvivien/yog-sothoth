import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  FEATURE_REGISTRY,
  parseStrictBoolean,
  isFeatureEnabled,
  type FeatureName,
} from "../features";

describe("parseStrictBoolean", () => {
  it("returns true for the literal string 'true'", () => {
    expect(parseStrictBoolean("true")).toBe(true);
  });

  it.each([
    ["undefined", undefined],
    ["empty string", ""],
    ["false", "false"],
    ["1", "1"],
    ["yes", "yes"],
    ["on", "on"],
    ["TRUE (uppercase)", "TRUE"],
    ["True (capitalized)", "True"],
    [" true (leading space)", " true"],
    ["true (trailing space)", "true "],
  ])("returns false for %s", (_label, input) => {
    expect(parseStrictBoolean(input)).toBe(false);
  });
});

describe("isFeatureEnabled (default registry values)", () => {
  // These tests verify the build-time defaults baked into the
  // registry. They do not exercise the env-var override path because
  // RAW_VALUES is captured at module import time and cannot be
  // mutated after the fact. Override behavior is tested separately
  // with module re-imports below.
  it("returns true for flags declared as defaultEnabled = true", () => {
    expect(isFeatureEnabled("poolsList")).toBe(true);
    expect(isFeatureEnabled("poolDetail")).toBe(true);
    expect(isFeatureEnabled("poolPriceImbalance")).toBe(true);
    expect(isFeatureEnabled("transactionFeed")).toBe(true);
  });

  it("returns false for flags declared as defaultEnabled = false", () => {
    expect(isFeatureEnabled("tvlTotal")).toBe(false);
    expect(isFeatureEnabled("alertsPanel")).toBe(false);
    expect(isFeatureEnabled("signalsFeed")).toBe(false);
  });
});

describe("FEATURE_REGISTRY shape", () => {
  it("declares a coherent entry for every flag", () => {
    const validStatuses = new Set([
      "available-v0.1",
      "degraded-v0.1",
      "pending-aggregate",
      "pending-signals",
      "pending-design",
      "pending-visual",
    ]);

    for (const [name, entry] of Object.entries(FEATURE_REGISTRY)) {
      expect(entry.description, `${name}.description`).toMatch(/.+/);
      expect(validStatuses.has(entry.status), `${name}.status`).toBe(true);
      expect(typeof entry.defaultEnabled, `${name}.defaultEnabled`).toBe(
        "boolean",
      );
    }
  });

  it("disables every flag whose status is not v0.1-ready", () => {
    // Sanity check on the registry: anything not marked as
    // available/degraded for v0.1 should default to off.
    const v0_1Ready = new Set(["available-v0.1", "degraded-v0.1"]);

    for (const [name, entry] of Object.entries(FEATURE_REGISTRY)) {
      if (!v0_1Ready.has(entry.status)) {
        expect(entry.defaultEnabled, `${name} should default to false`).toBe(
          false,
        );
      }
    }
  });
});

describe("isFeatureEnabled (env-var override)", () => {
  // These tests reload the module after mutating process.env so the
  // module-level RAW_VALUES map captures the patched values.
  const originalEnv = { ...process.env };

  beforeEach(() => {
    vi.resetModules();
  });

  afterEach(() => {
    process.env = { ...originalEnv };
  });

  async function loadModuleWithEnv(
    overrides: Record<string, string | undefined>,
  ): Promise<typeof import("../features")> {
    for (const [key, value] of Object.entries(overrides)) {
      if (value === undefined) {
        delete process.env[key];
      } else {
        process.env[key] = value;
      }
    }
    return import("../features");
  }

  it("flips a default-on flag to off when env var is 'false'", async () => {
    const mod = await loadModuleWithEnv({
      NEXT_PUBLIC_FEATURE_POOLS_LIST: "false",
    });
    expect(mod.isFeatureEnabled("poolsList")).toBe(false);
  });

  it("flips a default-off flag to on when env var is 'true'", async () => {
    const mod = await loadModuleWithEnv({
      NEXT_PUBLIC_FEATURE_TVL_TOTAL: "true",
    });
    expect(mod.isFeatureEnabled("tvlTotal")).toBe(true);
  });

  it("treats non-strict truthy values as off", async () => {
    const mod = await loadModuleWithEnv({
      NEXT_PUBLIC_FEATURE_TVL_TOTAL: "1",
    });
    expect(mod.isFeatureEnabled("tvlTotal")).toBe(false);
  });

  it("treats an unset env var as the registry default", async () => {
    const mod = await loadModuleWithEnv({
      NEXT_PUBLIC_FEATURE_POOLS_LIST: undefined,
    });
    expect(mod.isFeatureEnabled("poolsList")).toBe(true);
  });

  it("treats an empty-string override as off (explicit but falsy)", async () => {
    const mod = await loadModuleWithEnv({
      NEXT_PUBLIC_FEATURE_POOLS_LIST: "",
    });
    expect(mod.isFeatureEnabled("poolsList")).toBe(false);
  });
});

describe("FeatureName exhaustiveness", () => {
  it("covers every key of FEATURE_REGISTRY", () => {
    // Compile-time check: this object must declare an entry for every
    // FeatureName, otherwise the test file fails to type-check.
    const seen: Record<FeatureName, true> = {
      poolsList: true,
      poolDetail: true,
      poolPriceImbalance: true,
      transactionFeed: true,
      tvlTotal: true,
      volume24h: true,
      fees24h: true,
      tvlChart: true,
      pairBreakdown: true,
      keyMetrics: true,
      liquidityMap: true,
      liquidityHeatmap: true,
      liveStatusBar: true,
      liquidityHealthScore: true,
      alertsPanel: true,
      signalsFeed: true,
    };

    expect(Object.keys(seen).sort()).toEqual(
      Object.keys(FEATURE_REGISTRY).sort(),
    );
  });
});