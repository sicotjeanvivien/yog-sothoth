/**
 * Tests for the wire schemas.
 *
 * These tests act as the executable counterpart to the comments in
 * `schemas.ts`: if the Rust side drifts from the shape captured here,
 * the suite fails fast with a precise zod issue.
 */

import { describe, expect, it } from "vitest";
import { PoolSchema } from "../schema/pool";
import { PoolsPageSchema } from "../schema/page";
import { ApiErrorBodySchema } from "../schema/api-error-body";


// A representative valid pool payload, copied from a real yog-api
// response shape. Tests mutate this base to exercise each failure mode.
function validPool() {
  return {
    "poolAddress": "8Pm2kZpnxD3hoMmt4bjStX2Pw2Z9abpbHzZxMPqxPmie",
    "protocol": "meteora_damm_v2",
    "tokenA": {
      "mint": "So11111111111111111111111111111111111111112",
      "symbol": "SOL",
      "name": "Wrapped SOL",
      "decimals": 9,
      "logoUri": "https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/So11111111111111111111111111111111111111112/logo.png",
      "price": {
        "usd": "85.819299811880730000",
        "source": "jupiter",
        "fetchedAt": "2026-05-25T12:17:17.479657Z"
      }
    },
    "tokenB": {
      "mint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
      "symbol": "USDC",
      "name": "USD Coin",
      "decimals": 6,
      "logoUri": "https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v/logo.png",
      "price": {
        "usd": "0.999668653937465800",
        "source": "jupiter",
        "fetchedAt": "2026-05-25T12:17:17.479657Z"
      }
    },
    "tvlUsd": "1332007.7148736200400326721044",
    "volume24hUsd": "47964.973514780605664520660399",
    "firstSeenAt": "2026-05-21T10:01:35.084596Z",
    "lastSeenAt": "2026-05-25T12:14:01.715170Z"
  };
}

describe("PoolSchema", () => {
  it("accepts a complete valid pool", () => {
    const parsed = PoolSchema.parse(validPool());
    expect(parsed.poolAddress).toBe("8Pm2kZpnxD3hoMmt4bjStX2Pw2Z9abpbHzZxMPqxPmie");
    expect(parsed.protocol).toBe("meteora_damm_v2");
  });

  it("accepts RFC3339 with a numeric timezone offset", () => {
    const parsed = PoolSchema.parse({
      ...validPool(),
      firstSeenAt: "2026-05-21T10:01:35.084596Z",
    });
    expect(parsed.firstSeenAt).toBe("2026-05-21T10:01:35.084596Z");
  });

  it("rejects an empty poolAddress", () => {
    expect(() =>
      PoolSchema.parse({ ...validPool(), poolAddress: "" }),
    ).toThrow();
  });

  it("rejects a non-RFC3339 timestamp", () => {
    expect(() =>
      PoolSchema.parse({ ...validPool(), firstSeenAt: "yesterday" }),
    ).toThrow();
  });

  it("rejects a missing field", () => {
    const { protocol, ...rest } = validPool();
    void protocol;
    expect(() => PoolSchema.parse(rest)).toThrow();
  });
});

describe("PoolsPageSchema", () => {
  it("accepts an empty page with null cursor", () => {
    const parsed = PoolsPageSchema.parse({ items: [], nextCursor: null });
    expect(parsed.items).toHaveLength(0);
    expect(parsed.nextCursor).toBeNull();
  });

  it("accepts a full page with an opaque cursor", () => {
    const parsed = PoolsPageSchema.parse({
      items: [validPool(), validPool()],
      nextCursor: "eyJmaXJzdF9zZWVuX2F0IjoiMjAyNi0wNS0wMVQwODozMDowMFoifQ",
    });
    expect(parsed.items).toHaveLength(2);
    expect(parsed.nextCursor).toMatch(/^[A-Za-z0-9_-]+$/);
  });

  it("rejects items that fail individual validation", () => {
    expect(() =>
      PoolsPageSchema.parse({
        items: [{ ...validPool(), poolAddress: "" }],
        nextCursor: null,
      }),
    ).toThrow();
  });

  it("rejects a missing nextCursor field", () => {
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