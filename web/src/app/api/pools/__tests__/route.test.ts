/**
 * Tests for the `GET /api/pools` BFF route handler.
 *
 * `fetchPools` is mocked because the full upstream chain is already
 * exercised by `lib/api/__tests__/pools.test.ts`. Here we isolate:
 *
 *   - query string parsing (limit out of range, non-integer, etc.)
 *   - happy path passthrough of the typed page
 *   - error mapping via `mapApiClientErrorToHttp`
 *
 * `vi.mock` is hoisted by vitest to the top of the file, *before*
 * any const declarations. To share a mock function between the
 * factory and the test bodies, we declare it via `vi.hoisted()` so
 * it lives in the same hoist scope.
 */

import { afterEach, describe, expect, it, vi } from "vitest";

import { ApiClientError } from "@/lib/api/errors";

// Hoisted alongside vi.mock — available inside both the factory and
// the test bodies.
const { fetchPoolsMock } = vi.hoisted(() => ({
  fetchPoolsMock: vi.fn(),
}));

// Mock the entire `pools` module so we control what the handler sees.
// `POOLS_QUERY_BOUNDS` is kept from the real module so the handler's
// input validation uses the real bounds.
vi.mock("@/lib/api/pools", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api/pools")>(
    "@/lib/api/pools",
  );
  return {
    ...actual,
    fetchPools: fetchPoolsMock,
  };
});

// Import the handler AFTER the mock is registered, so it picks up the
// mocked `fetchPools` rather than the real one.
import { GET } from "../route";

afterEach(() => {
  fetchPoolsMock.mockReset();
});

/** Build a Request targeting /api/pools with the given query params. */
function makeRequest(query: string = ""): Request {
  const qs = query ? `?${query}` : "";
  return new Request(`http://localhost:3000/api/pools${qs}`);
}

// ─────────────────────────────────────────────────────────────────────
// Happy path
// ─────────────────────────────────────────────────────────────────────

describe("GET /api/pools — happy path", () => {
  it("returns the typed page on success", async () => {
    const page = { items: [], next_cursor: null };
    fetchPoolsMock.mockResolvedValue(page);

    const response = await GET(makeRequest());
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual(page);

    // Default limit was applied, no cursor key emitted (the handler
    // omits the key entirely when the cursor is absent, per
    // exactOptionalPropertyTypes).
    expect(fetchPoolsMock).toHaveBeenCalledWith({ limit: 50 });
  });

  it("forwards a custom limit and cursor", async () => {
    fetchPoolsMock.mockResolvedValue({ items: [], next_cursor: "abc" });

    await GET(makeRequest("limit=25&cursor=xyz"));

    expect(fetchPoolsMock).toHaveBeenCalledWith({
      cursor: "xyz",
      limit: 25,
    });
  });

  it("treats an empty cursor as absent (key omitted)", async () => {
    fetchPoolsMock.mockResolvedValue({ items: [], next_cursor: null });

    await GET(makeRequest("cursor="));

    // Empty cursor → key absent, not `cursor: undefined`.
    expect(fetchPoolsMock).toHaveBeenCalledWith({ limit: 50 });
    const callArg = fetchPoolsMock.mock.calls[0]?.[0] as Record<string, unknown>;
    expect("cursor" in callArg).toBe(false);
  });
});

// ─────────────────────────────────────────────────────────────────────
// Input validation — returns 400 without calling fetchPools
// ─────────────────────────────────────────────────────────────────────

describe("GET /api/pools — input validation", () => {
  it("returns 400 when limit is not an integer", async () => {
    const response = await GET(makeRequest("limit=abc"));
    expect(response.status).toBe(400);

    const body = await response.json();
    expect(body.kind).toBe("bad_request");
    expect(body.error).toMatch(/integer/);
    expect(fetchPoolsMock).not.toHaveBeenCalled();
  });

  it("returns 400 when limit is a float", async () => {
    const response = await GET(makeRequest("limit=12.5"));
    expect(response.status).toBe(400);
    expect(fetchPoolsMock).not.toHaveBeenCalled();
  });

  it("returns 400 when limit is below 1", async () => {
    const response = await GET(makeRequest("limit=0"));
    expect(response.status).toBe(400);

    const body = await response.json();
    expect(body.error).toMatch(/between 1 and 200/);
  });

  it("returns 400 when limit is above the maximum", async () => {
    const response = await GET(makeRequest("limit=201"));
    expect(response.status).toBe(400);
  });
});

// ─────────────────────────────────────────────────────────────────────
// Error mapping — each ApiClientError variant
// ─────────────────────────────────────────────────────────────────────

describe("GET /api/pools — error mapping", () => {
  it("maps a timeout to 504", async () => {
    fetchPoolsMock.mockRejectedValue(ApiClientError.timeout(5000));

    const response = await GET(makeRequest());
    expect(response.status).toBe(504);
    expect((await response.json()).kind).toBe("gateway_timeout");
  });

  it("maps a network failure to 502", async () => {
    fetchPoolsMock.mockRejectedValue(
      ApiClientError.network(new TypeError("fetch failed")),
    );

    const response = await GET(makeRequest());
    expect(response.status).toBe(502);
    expect((await response.json()).kind).toBe("bad_gateway");
  });

  it("passes through a 4xx from yog-api", async () => {
    fetchPoolsMock.mockRejectedValue(
      ApiClientError.http(400, "invalid cursor"),
    );

    const response = await GET(makeRequest());
    expect(response.status).toBe(400);

    const body = await response.json();
    expect(body.kind).toBe("bad_request");
    expect(body.error).toBe("invalid cursor");
  });

  it("collapses a 5xx from yog-api into 502", async () => {
    fetchPoolsMock.mockRejectedValue(
      ApiClientError.http(500, "internal server error"),
    );

    const response = await GET(makeRequest());
    expect(response.status).toBe(502);

    const body = await response.json();
    // The internal yog-api 500 message must NOT leak to the browser.
    expect(body.error).not.toMatch(/internal server error/);
  });

  it("maps a schema validation failure to 502", async () => {
    fetchPoolsMock.mockRejectedValue(
      ApiClientError.validation(["items: expected array"]),
    );

    const response = await GET(makeRequest());
    expect(response.status).toBe(502);

    const body = await response.json();
    // Internal zod issues must NOT leak.
    expect(body.error).not.toMatch(/items/);
  });

  it("returns 500 on an unexpected non-ApiClientError throw", async () => {
    fetchPoolsMock.mockRejectedValue(new Error("boom"));

    const response = await GET(makeRequest());
    expect(response.status).toBe(500);
  });
});