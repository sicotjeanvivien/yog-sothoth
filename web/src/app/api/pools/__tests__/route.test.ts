/**
 * Tests for the `GET /api/pools` BFF route handler.
 *
 * `fetchPools` is mocked because the full upstream chain is already
 * exercised by `lib/api/__tests__/pools.test.ts`. Here we isolate:
 *
 *   - query string parsing (limit out of range, non-integer, etc.)
 *   - happy path passthrough of the typed page
 *   - error mapping via `mapApiClientErrorToHttp`
 *   - RFC 9457 wire contract on every error response (four required
 *     fields, correct Content-Type)
 *
 * `vi.mock` is hoisted by vitest to the top of the file, *before*
 * any const declarations. To share a mock function between the
 * factory and the test bodies, we declare it via `vi.hoisted()` so
 * it lives in the same hoist scope.
 */

import { afterEach, describe, expect, it, vi } from "vitest";

import { ApiClientError } from "@/lib/api/errors";
import { PROBLEM_CONTENT_TYPE } from "@/lib/api/http-mapping";

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
    // Success uses application/json, NOT application/problem+json.
    expect(response.headers.get("content-type")).toMatch(/application\/json/);
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
// Input validation — returns 400 Problem Details without calling fetchPools
// ─────────────────────────────────────────────────────────────────────

describe("GET /api/pools — input validation", () => {
  it("returns 400 Problem Details when limit is not an integer", async () => {
    const response = await GET(makeRequest("limit=abc"));
    expect(response.status).toBe(400);
    expect(response.headers.get("content-type")).toBe(PROBLEM_CONTENT_TYPE);

    const body = await response.json();
    expect(body.title).toBe("Bad Request");
    expect(body.status).toBe(400);
    expect(body.type).toBe("about:blank");
    expect(body.detail).toMatch(/integer/);
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
    expect(body.detail).toMatch(/between 1 and 200/);
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
  it("maps a timeout to 504 Gateway Timeout Problem Details", async () => {
    fetchPoolsMock.mockRejectedValue(ApiClientError.timeout(5000));

    const response = await GET(makeRequest());
    expect(response.status).toBe(504);
    expect(response.headers.get("content-type")).toBe(PROBLEM_CONTENT_TYPE);

    const body = await response.json();
    expect(body.title).toBe("Gateway Timeout");
    expect(body.status).toBe(504);
  });

  it("maps a network failure to 502 Bad Gateway", async () => {
    fetchPoolsMock.mockRejectedValue(
      ApiClientError.network(new TypeError("fetch failed")),
    );

    const response = await GET(makeRequest());
    expect(response.status).toBe(502);

    const body = await response.json();
    expect(body.title).toBe("Bad Gateway");
  });

  it("passes through a 4xx from yog-api", async () => {
    fetchPoolsMock.mockRejectedValue(
      ApiClientError.http(400, "invalid cursor"),
    );

    const response = await GET(makeRequest());
    expect(response.status).toBe(400);

    const body = await response.json();
    expect(body.title).toBe("Bad Request");
    expect(body.detail).toBe("invalid cursor");
  });

  it("collapses a 5xx from yog-api into 502", async () => {
    fetchPoolsMock.mockRejectedValue(
      ApiClientError.http(500, "internal server error"),
    );

    const response = await GET(makeRequest());
    expect(response.status).toBe(502);

    const body = await response.json();
    expect(body.title).toBe("Bad Gateway");
    // The internal yog-api 500 message must NOT leak to the browser.
    expect(body.detail).not.toMatch(/internal server error/);
  });

  it("maps a schema validation failure to 502", async () => {
    fetchPoolsMock.mockRejectedValue(
      ApiClientError.validation(["items: expected array"]),
    );

    const response = await GET(makeRequest());
    expect(response.status).toBe(502);

    const body = await response.json();
    expect(body.title).toBe("Bad Gateway");
    // Internal zod issues must NOT leak.
    expect(body.detail).not.toMatch(/items/);
  });

  it("returns 500 Internal Server Error on an unexpected non-ApiClientError throw", async () => {
    fetchPoolsMock.mockRejectedValue(new Error("boom"));

    const response = await GET(makeRequest());
    // Was 500 with kind: "bad_gateway" before (inconsistent) — now
    // 500 with title: "Internal Server Error" (coherent).
    expect(response.status).toBe(500);
    expect(response.headers.get("content-type")).toBe(PROBLEM_CONTENT_TYPE);

    const body = await response.json();
    expect(body.title).toBe("Internal Server Error");
    expect(body.status).toBe(500);
  });
});