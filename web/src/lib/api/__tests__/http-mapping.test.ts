/**
 * Tests for `mapApiClientErrorToHttp` and the Problem Details
 * helpers.
 *
 * Each variant of `ApiClientError` maps to exactly one HTTP shape;
 * the 4xx/5xx branching inside `http` is the only conditional.
 * Tests pin the RFC 9457 wire contract (four required fields, stable
 * `title` per kind, `type` = "about:blank") so a refactor cannot
 * silently change the public format.
 */

import { describe, expect, it } from "vitest";

import { ApiClientError } from "../errors";
import {
  badRequestProblem,
  internalErrorProblem,
  mapApiClientErrorToHttp,
  PROBLEM_CONTENT_TYPE,
  problemResponse,
} from "../http-mapping";

// ─────────────────────────────────────────────────────────────────────
// ApiClientError → Problem Details mapping
// ─────────────────────────────────────────────────────────────────────

describe("mapApiClientErrorToHttp", () => {
  it("maps timeout to 504 Gateway Timeout", () => {
    const { status, body } = mapApiClientErrorToHttp(ApiClientError.timeout(5000));
    expect(status).toBe(504);
    expect(body.title).toBe("Gateway Timeout");
    expect(body.status).toBe(504);
    expect(body.detail).toBe("upstream API timed out");
  });

  it("maps network failure to 502 Bad Gateway", () => {
    const { status, body } = mapApiClientErrorToHttp(
      ApiClientError.network(new TypeError("fetch failed")),
    );
    expect(status).toBe(502);
    expect(body.title).toBe("Bad Gateway");
    expect(body.status).toBe(502);
  });

  it("maps validation failure to 502 Bad Gateway", () => {
    const { status, body } = mapApiClientErrorToHttp(
      ApiClientError.validation(["items: expected array"]),
    );
    expect(status).toBe(502);
    expect(body.title).toBe("Bad Gateway");
    // Internal zod issues must NOT leak to the browser.
    expect(body.detail).not.toMatch(/items/);
  });

  it("passes through a 400 from yog-api as Bad Request", () => {
    const { status, body } = mapApiClientErrorToHttp(
      ApiClientError.http(400, "limit out of range"),
    );
    expect(status).toBe(400);
    expect(body.title).toBe("Bad Request");
    expect(body.status).toBe(400);
    expect(body.detail).toBe("limit out of range");
  });

  it("passes through a 404 from yog-api as Not Found", () => {
    const { status, body } = mapApiClientErrorToHttp(
      ApiClientError.http(404, "pool not found"),
    );
    expect(status).toBe(404);
    expect(body.title).toBe("Not Found");
    expect(body.status).toBe(404);
    expect(body.detail).toBe("pool not found");
  });

  it("uses a generic detail when 4xx has no remote message", () => {
    const { body } = mapApiClientErrorToHttp(ApiClientError.http(400, null));
    expect(body.detail).toBe("request rejected by upstream API");
  });

  it("collapses a 500 from yog-api into a 502 Bad Gateway", () => {
    const { status, body } = mapApiClientErrorToHttp(
      ApiClientError.http(500, "internal server error"),
    );
    expect(status).toBe(502);
    expect(body.title).toBe("Bad Gateway");
    // The remote message from yog-api's 500 is intentionally dropped.
    expect(body.detail).not.toMatch(/internal server error/);
  });

  it("collapses a 503 from yog-api into a 502 Bad Gateway", () => {
    const { status, body } = mapApiClientErrorToHttp(
      ApiClientError.http(503, "service unavailable"),
    );
    expect(status).toBe(502);
    expect(body.title).toBe("Bad Gateway");
  });
});

// ─────────────────────────────────────────────────────────────────────
// RFC 9457 wire contract
// ─────────────────────────────────────────────────────────────────────

describe("RFC 9457 wire contract", () => {
  it("every mapped body carries the four required fields", () => {
    // Sample one body of each branch. Tautological for the others.
    const samples = [
      mapApiClientErrorToHttp(ApiClientError.timeout(5000)).body,
      mapApiClientErrorToHttp(ApiClientError.network(new TypeError("x"))).body,
      mapApiClientErrorToHttp(ApiClientError.validation(["x"])).body,
      mapApiClientErrorToHttp(ApiClientError.http(404, "x")).body,
      mapApiClientErrorToHttp(ApiClientError.http(500, "x")).body,
    ];

    for (const body of samples) {
      expect(body).toHaveProperty("type");
      expect(body).toHaveProperty("title");
      expect(body).toHaveProperty("status");
      expect(body).toHaveProperty("detail");
      expect(typeof body.status).toBe("number");
    }
  });

  it("every mapped body uses type = about:blank at this stage", () => {
    // When specific type URIs are introduced later, this expectation
    // will change accordingly. Pinning the current contract.
    const samples = [
      mapApiClientErrorToHttp(ApiClientError.timeout(5000)).body,
      mapApiClientErrorToHttp(ApiClientError.http(400, "x")).body,
    ];
    for (const body of samples) {
      expect(body.type).toBe("about:blank");
    }
  });

  it("title is stable across two occurrences of the same kind", () => {
    // The browser uses (status, title) for branching, so the same
    // failure kind must produce the same title every time.
    const t1 = mapApiClientErrorToHttp(ApiClientError.timeout(1000)).body.title;
    const t2 = mapApiClientErrorToHttp(ApiClientError.timeout(9999)).body.title;
    expect(t1).toBe(t2);
    expect(t1).toBe("Gateway Timeout");
  });
});

// ─────────────────────────────────────────────────────────────────────
// Helpers for local validation / unexpected errors
// ─────────────────────────────────────────────────────────────────────

describe("badRequestProblem", () => {
  it("builds a 400 problem carrying the provided detail", () => {
    const body = badRequestProblem("`limit` must be an integer");
    expect(body.status).toBe(400);
    expect(body.title).toBe("Bad Request");
    expect(body.type).toBe("about:blank");
    expect(body.detail).toBe("`limit` must be an integer");
  });
});

describe("internalErrorProblem", () => {
  it("builds a 500 problem with a generic detail", () => {
    const body = internalErrorProblem();
    expect(body.status).toBe(500);
    expect(body.title).toBe("Internal Server Error");
    expect(body.detail).toBe("internal server error");
  });
});

// ─────────────────────────────────────────────────────────────────────
// problemResponse helper
// ─────────────────────────────────────────────────────────────────────

describe("problemResponse", () => {
  it("sets the RFC 9457 content type", () => {
    const body = badRequestProblem("nope");
    const response = problemResponse(body);
    expect(response.headers.get("content-type")).toBe(PROBLEM_CONTENT_TYPE);
  });

  it("uses the body status when init.status is omitted", () => {
    const body = badRequestProblem("nope");
    const response = problemResponse(body);
    expect(response.status).toBe(400);
  });

  it("init.status overrides the body status", () => {
    // Useful when the body status is set generically (e.g. 502 from
    // the mapping) but the caller wants to override for a specific
    // route — not used today but exercising the contract.
    const body = badRequestProblem("nope");
    const response = problemResponse(body, { status: 418 });
    expect(response.status).toBe(418);
  });

  it("serialises the body as JSON", async () => {
    const body = badRequestProblem("invalid cursor");
    const response = problemResponse(body);
    const parsed = await response.json();
    expect(parsed).toEqual({
      type: "about:blank",
      title: "Bad Request",
      status: 400,
      detail: "invalid cursor",
    });
  });
});