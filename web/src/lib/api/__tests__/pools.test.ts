/**
 * Integration tests for `fetchPools`.
 *
 * The unit under test is the full chain: input validation → URL build
 * (from env) → fetch call → status check → schema parsing. The only
 * thing mocked is `globalThis.fetch` and the env singleton — every
 * other module loads its real implementation, so a regression in
 * `client.ts` or `schemas.ts` shows up here too.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ApiClientError } from "../errors";
import { fetchPools } from "../pools";
import { __resetServerEnv } from "../../config/server-env.schema";
import { validPoolsPage } from "./fixtures";

// Configure a deterministic env for the suite. We rebuild the cache by
// resetting the singleton before each test, after touching process.env.
const TEST_API_BASE_URL = "http://api.test";
const TEST_API_TIMEOUT_MS = "5000";

beforeEach(() => {
  process.env.YOG_API_BASE_URL = TEST_API_BASE_URL;
  process.env.YOG_API_TIMEOUT_MS = TEST_API_TIMEOUT_MS;
  __resetServerEnv();
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

// ─────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────

/** Build a `Response`-like object that vitest can return from fetch. */
function jsonResponse(body: unknown, status: number = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}


// ─────────────────────────────────────────────────────────────────────
// Happy path
// ─────────────────────────────────────────────────────────────────────

describe("fetchPools — happy path", () => {
  it("calls yog-api with the default limit and parses the response", async () => {
    const fetchSpy = vi.fn().mockResolvedValue(jsonResponse(validPoolsPage({ nextCursor: null })));
    vi.stubGlobal("fetch", fetchSpy);

    const page = await fetchPools();

    expect(page.items).toHaveLength(1);
    expect(page.nextCursor).toBeNull();

    // Verify the URL composition is correct: base + path + default limit.
    expect(fetchSpy).toHaveBeenCalledOnce();
    const calledUrl = String(fetchSpy.mock.calls[0]?.[0] ?? "");
    expect(calledUrl).toBe(`${TEST_API_BASE_URL}/api/pools?limit=50`);
  });

  it("forwards a custom limit and cursor", async () => {
    const fetchSpy = vi.fn().mockResolvedValue(jsonResponse(validPoolsPage({ nextCursor: "abc123" })));
    vi.stubGlobal("fetch", fetchSpy);

    await fetchPools({ cursor: "abc123", limit: 25 });

    const calledUrl = new URL(String(fetchSpy.mock.calls[0]?.[0] ?? ""));
    expect(calledUrl.searchParams.get("limit")).toBe("25");
    expect(calledUrl.searchParams.get("cursor")).toBe("abc123");
  });

  it("drops an empty cursor instead of sending an empty string", async () => {
    const fetchSpy = vi.fn().mockResolvedValue(jsonResponse(validPoolsPage()));
    vi.stubGlobal("fetch", fetchSpy);

    await fetchPools({ cursor: "" });

    const calledUrl = new URL(String(fetchSpy.mock.calls[0]?.[0] ?? ""));
    expect(calledUrl.searchParams.has("cursor")).toBe(false);
  });

  it("exposes the full pagination metadata to the caller", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        jsonResponse(
          validPoolsPage({
            nextCursor: "next-x",
            prevCursor: "prev-y",
            isFirst: false,
            isLast: false,
          }),
        ),
      ),
    );

    const page = await fetchPools();

    expect(page.nextCursor).toBe("next-x");
    expect(page.prevCursor).toBe("prev-y");
    expect(page.isFirst).toBe(false);
    expect(page.isLast).toBe(false);
  });
});

describe("fetchPools — bidirectional pagination params", () => {
  it("forwards dir=next", async () => {
    const fetchSpy = vi.fn().mockResolvedValue(jsonResponse(validPoolsPage()));
    vi.stubGlobal("fetch", fetchSpy);

    await fetchPools({ cursor: "abc", dir: "next" });

    const calledUrl = new URL(String(fetchSpy.mock.calls[0]?.[0] ?? ""));
    expect(calledUrl.searchParams.get("dir")).toBe("next");
    expect(calledUrl.searchParams.get("cursor")).toBe("abc");
  });

  it("forwards dir=prev", async () => {
    const fetchSpy = vi.fn().mockResolvedValue(jsonResponse(validPoolsPage()));
    vi.stubGlobal("fetch", fetchSpy);

    await fetchPools({ cursor: "abc", dir: "prev" });

    const calledUrl = new URL(String(fetchSpy.mock.calls[0]?.[0] ?? ""));
    expect(calledUrl.searchParams.get("dir")).toBe("prev");
  });

  it("forwards position=first", async () => {
    const fetchSpy = vi.fn().mockResolvedValue(jsonResponse(validPoolsPage()));
    vi.stubGlobal("fetch", fetchSpy);

    await fetchPools({ position: "first" });

    const calledUrl = new URL(String(fetchSpy.mock.calls[0]?.[0] ?? ""));
    expect(calledUrl.searchParams.get("position")).toBe("first");
  });

  it("forwards position=last", async () => {
    const fetchSpy = vi.fn().mockResolvedValue(jsonResponse(validPoolsPage()));
    vi.stubGlobal("fetch", fetchSpy);

    await fetchPools({ position: "last" });

    const calledUrl = new URL(String(fetchSpy.mock.calls[0]?.[0] ?? ""));
    expect(calledUrl.searchParams.get("position")).toBe("last");
  });

  it("omits dir when not specified", async () => {
    const fetchSpy = vi.fn().mockResolvedValue(jsonResponse(validPoolsPage()));
    vi.stubGlobal("fetch", fetchSpy);

    await fetchPools({ cursor: "abc" });

    const calledUrl = new URL(String(fetchSpy.mock.calls[0]?.[0] ?? ""));
    expect(calledUrl.searchParams.has("dir")).toBe(false);
  });

  it("omits position when not specified", async () => {
    const fetchSpy = vi.fn().mockResolvedValue(jsonResponse(validPoolsPage()));
    vi.stubGlobal("fetch", fetchSpy);

    await fetchPools();

    const calledUrl = new URL(String(fetchSpy.mock.calls[0]?.[0] ?? ""));
    expect(calledUrl.searchParams.has("position")).toBe(false);
  });
});

// ─────────────────────────────────────────────────────────────────────
// Input validation (RangeError, not ApiClientError)
// ─────────────────────────────────────────────────────────────────────

describe("fetchPools — input validation", () => {
  it("rejects limit = 0", async () => {
    await expect(fetchPools({ limit: 0 })).rejects.toThrow(RangeError);
  });

  it("rejects limit above the maximum", async () => {
    await expect(fetchPools({ limit: 201 })).rejects.toThrow(RangeError);
  });

  it("rejects a non-integer limit", async () => {
    await expect(fetchPools({ limit: 25.5 })).rejects.toThrow(RangeError);
  });
});

// ─────────────────────────────────────────────────────────────────────
// ApiClientError variants
// ─────────────────────────────────────────────────────────────────────

describe("fetchPools — HTTP failures", () => {
  it("maps a 400 with a typed body to ApiClientError(http)", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(jsonResponse({ error: "limit out of range" }, 400)),
    );

    try {
      await fetchPools();
      expect.fail("expected ApiClientError");
    } catch (err) {
      expect(err).toBeInstanceOf(ApiClientError);
      expect((err as ApiClientError).details).toEqual({
        kind: "http",
        status: 400,
        remoteMessage: "limit out of range",
      });
    }
  });

  it("maps a 500 with no body to ApiClientError(http) with a null remoteMessage", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(null, { status: 500 }),
      ),
    );

    try {
      await fetchPools();
      expect.fail("expected ApiClientError");
    } catch (err) {
      expect(err).toBeInstanceOf(ApiClientError);
      const details = (err as ApiClientError).details;
      expect(details.kind).toBe("http");
      if (details.kind === "http") {
        expect(details.status).toBe(500);
        expect(details.remoteMessage).toBeNull();
      }
    }
  });
});

describe("fetchPools — schema failures", () => {
  it("maps a 200 with a malformed body to ApiClientError(validation)", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        jsonResponse({ items: "not an array", nextCursor: null }),
      ),
    );

    try {
      await fetchPools();
      expect.fail("expected ApiClientError");
    } catch (err) {
      expect(err).toBeInstanceOf(ApiClientError);
      expect((err as ApiClientError).details.kind).toBe("validation");
    }
  });

  it("maps a 200 with invalid JSON to ApiClientError(validation)", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response("not json at all", {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      ),
    );

    try {
      await fetchPools();
      expect.fail("expected ApiClientError");
    } catch (err) {
      expect(err).toBeInstanceOf(ApiClientError);
      expect((err as ApiClientError).details.kind).toBe("validation");
    }
  });
});

describe("fetchPools — transport failures", () => {
  it("maps an AbortSignal TimeoutError to ApiClientError(timeout)", async () => {
    // Simulate what `AbortSignal.timeout` rejects with: a DOMException
    // whose name is "TimeoutError".
    vi.stubGlobal(
      "fetch",
      vi.fn().mockRejectedValue(new DOMException("aborted", "TimeoutError")),
    );

    try {
      await fetchPools();
      expect.fail("expected ApiClientError");
    } catch (err) {
      expect(err).toBeInstanceOf(ApiClientError);
      expect((err as ApiClientError).details).toEqual({
        kind: "timeout",
        timeoutMs: 5000,
      });
    }
  });

  it("maps a generic fetch failure to ApiClientError(network)", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockRejectedValue(new TypeError("fetch failed")),
    );

    try {
      await fetchPools();
      expect.fail("expected ApiClientError");
    } catch (err) {
      expect(err).toBeInstanceOf(ApiClientError);
      expect((err as ApiClientError).details.kind).toBe("network");
    }
  });
});