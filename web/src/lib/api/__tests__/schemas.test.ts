/**
 * Tests for the wire schemas.
 *
 * These tests act as the executable counterpart to the comments in
 * `schemas.ts`: if the Rust side drifts from the shape captured here,
 * the suite fails fast with a precise zod issue.
 */

import { describe, expect, it } from "vitest";
import { PoolResponseSchema } from "../schema/pool-response";
import { PoolsPageSchema } from "../schema/page-response";
import { ApiErrorBodySchema } from "../schema/api-error-body";


// A representative valid pool payload, copied from a real yog-api
// response shape. Tests mutate this base to exercise each failure mode.
function validPool() {
  return {
    pool_address: "CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j",
    protocol: "damm_v2",
    token_a_mint: "So11111111111111111111111111111111111111112",
    token_b_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
    first_seen_at: "2026-05-01T08:30:00Z",
    last_seen_at: "2026-05-12T03:18:42.515Z",
  };
}

describe("PoolResponseSchema", () => {
  it("accepts a complete valid pool", () => {
    const parsed = PoolResponseSchema.parse(validPool());
    expect(parsed.pool_address).toBe("CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j");
    expect(parsed.protocol).toBe("damm_v2");
  });

  it("accepts RFC3339 with a numeric timezone offset", () => {
    const parsed = PoolResponseSchema.parse({
      ...validPool(),
      first_seen_at: "2026-05-01T08:30:00+02:00",
    });
    expect(parsed.first_seen_at).toBe("2026-05-01T08:30:00+02:00");
  });

  it("rejects an empty pool_address", () => {
    expect(() =>
      PoolResponseSchema.parse({ ...validPool(), pool_address: "" }),
    ).toThrow();
  });

  it("rejects a non-RFC3339 timestamp", () => {
    expect(() =>
      PoolResponseSchema.parse({ ...validPool(), first_seen_at: "yesterday" }),
    ).toThrow();
  });

  it("rejects a missing field", () => {
    const { protocol, ...rest } = validPool();
    void protocol;
    expect(() => PoolResponseSchema.parse(rest)).toThrow();
  });
});

describe("PoolsPageSchema", () => {
  it("accepts an empty page with null cursor", () => {
    const parsed = PoolsPageSchema.parse({ items: [], next_cursor: null });
    expect(parsed.items).toHaveLength(0);
    expect(parsed.next_cursor).toBeNull();
  });

  it("accepts a full page with an opaque cursor", () => {
    const parsed = PoolsPageSchema.parse({
      items: [validPool(), validPool()],
      next_cursor: "eyJmaXJzdF9zZWVuX2F0IjoiMjAyNi0wNS0wMVQwODozMDowMFoifQ",
    });
    expect(parsed.items).toHaveLength(2);
    expect(parsed.next_cursor).toMatch(/^[A-Za-z0-9_-]+$/);
  });

  it("rejects items that fail individual validation", () => {
    expect(() =>
      PoolsPageSchema.parse({
        items: [{ ...validPool(), pool_address: "" }],
        next_cursor: null,
      }),
    ).toThrow();
  });

  it("rejects a missing next_cursor field", () => {
    expect(() => PoolsPageSchema.parse({ items: [] })).toThrow();
  });
});

describe("ApiErrorBodySchema", () => {
  it("accepts a valid error envelope", () => {
    const parsed = ApiErrorBodySchema.parse({ error: "limit out of range" });
    expect(parsed.error).toBe("limit out of range");
  });

  it("rejects a missing error field", () => {
    expect(() => ApiErrorBodySchema.parse({})).toThrow();
  });

  it("rejects a non-string error field", () => {
    expect(() => ApiErrorBodySchema.parse({ error: 42 })).toThrow();
  });
});