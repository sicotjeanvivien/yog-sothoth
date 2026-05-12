/**
 * Tests for `mapApiClientErrorToHttp`.
 *
 * Each variant of `ApiClientError` maps to exactly one HTTP shape;
 * the 4xx/5xx branching inside `http` is the only conditional.
 */

import { describe, expect, it } from "vitest";

import { ApiClientError } from "../errors";
import { mapApiClientErrorToHttp } from "../http-mapping";

describe("mapApiClientErrorToHttp", () => {
  it("maps timeout to 504 gateway_timeout", () => {
    const { status, body } = mapApiClientErrorToHttp(ApiClientError.timeout(5000));
    expect(status).toBe(504);
    expect(body.kind).toBe("gateway_timeout");
  });

  it("maps network failure to 502 bad_gateway", () => {
    const { status, body } = mapApiClientErrorToHttp(
      ApiClientError.network(new TypeError("fetch failed")),
    );
    expect(status).toBe(502);
    expect(body.kind).toBe("bad_gateway");
  });

  it("maps validation failure to 502 bad_gateway", () => {
    const { status, body } = mapApiClientErrorToHttp(
      ApiClientError.validation(["items: expected array"]),
    );
    expect(status).toBe(502);
    expect(body.kind).toBe("bad_gateway");
    // Internal zod issues must NOT leak to the browser.
    expect(body.error).not.toMatch(/items/);
  });

  it("passes through a 400 from yog-api as bad_request", () => {
    const { status, body } = mapApiClientErrorToHttp(
      ApiClientError.http(400, "limit out of range"),
    );
    expect(status).toBe(400);
    expect(body.kind).toBe("bad_request");
    expect(body.error).toBe("limit out of range");
  });

  it("passes through a 404 from yog-api as not_found", () => {
    const { status, body } = mapApiClientErrorToHttp(
      ApiClientError.http(404, "pool not found"),
    );
    expect(status).toBe(404);
    expect(body.kind).toBe("not_found");
    expect(body.error).toBe("pool not found");
  });

  it("uses a generic message when 4xx has no remote message", () => {
    const { body } = mapApiClientErrorToHttp(ApiClientError.http(400, null));
    expect(body.error).toBe("request rejected by upstream API");
  });

  it("collapses a 500 from yog-api into a 502 bad_gateway", () => {
    const { status, body } = mapApiClientErrorToHttp(
      ApiClientError.http(500, "internal server error"),
    );
    expect(status).toBe(502);
    expect(body.kind).toBe("bad_gateway");
    // The remote message from yog-api's 500 is intentionally dropped.
    expect(body.error).not.toMatch(/internal server error/);
  });

  it("collapses a 503 from yog-api into a 502 bad_gateway", () => {
    const { status, body } = mapApiClientErrorToHttp(
      ApiClientError.http(503, "service unavailable"),
    );
    expect(status).toBe(502);
    expect(body.kind).toBe("bad_gateway");
  });
});