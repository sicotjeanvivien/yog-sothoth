/**
 * Integration tests for `fetchTopPools`.
 *
 * Covers the metric → query-param mapping: the `volume_24h` default is sent
 * implicitly (param omitted, so the URL stays bare), and `tvl` is forwarded
 * as `metric=tvl`. The full chain (URL build from env → fetch → schema parse)
 * runs for real; only `globalThis.fetch` and the env singleton are stubbed.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { fetchTopPools } from "../server/top-pools";
import { __resetServerEnv } from "../../config/server-env.schema";
import { validPool } from "./fixtures";

const TEST_API_BASE_URL = "http://api.test";

beforeEach(() => {
  process.env.YOG_API_INTERNAL_URL = TEST_API_BASE_URL;
  process.env.YOG_API_TIMEOUT_MS = "5000";
  __resetServerEnv();
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

describe("fetchTopPools — metric param", () => {
  it("omits the metric for the volume default (bare URL)", async () => {
    const fetchSpy = vi.fn().mockResolvedValue(jsonResponse([validPool()]));
    vi.stubGlobal("fetch", fetchSpy);

    const pools = await fetchTopPools();

    expect(pools).toHaveLength(1);
    const url = new URL(String(fetchSpy.mock.calls[0]?.[0] ?? ""));
    expect(url.pathname).toBe("/api/pools/top");
    expect(url.searchParams.has("metric")).toBe(false);
  });

  it("forwards metric=tvl", async () => {
    const fetchSpy = vi.fn().mockResolvedValue(jsonResponse([validPool()]));
    vi.stubGlobal("fetch", fetchSpy);

    await fetchTopPools("tvl");

    const url = new URL(String(fetchSpy.mock.calls[0]?.[0] ?? ""));
    expect(url.searchParams.get("metric")).toBe("tvl");
  });

  it("omits the metric when volume_24h is passed explicitly", async () => {
    const fetchSpy = vi.fn().mockResolvedValue(jsonResponse([validPool()]));
    vi.stubGlobal("fetch", fetchSpy);

    await fetchTopPools("volume_24h");

    const url = new URL(String(fetchSpy.mock.calls[0]?.[0] ?? ""));
    expect(url.searchParams.has("metric")).toBe(false);
  });
});
