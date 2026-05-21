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
    "pool_address": "BhVFo9nCA9X45yUUa7QgwUkR4mZcAop2kytSNhmQiS4C",
    "protocol": "meteora_damm_v2",
    "token_a": {
      "mint": "CMButZqQKoRabRAwemmG9gpXKa62KpQByLwjQLbjM1US",
      "symbol": "SAOS",
      "name": "Strategic American Oil Supply",
      "decimals": 6,
      "logoUri": "https://known-sapphire-boa.myfilebase.com/ipfs/QmQzbdyPhKHR2R8WPda5b3D7WHh55oDednv3ChYYSMRKuy",
      "price": {
        "usd": "0.005746334785293797",
        "source": "jupiter",
        "fetchedAt": "2026-05-21T10:06:53.095599Z"
      }
    },
    "token_b": {
      "mint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
      "symbol": "USDC",
      "name": "USD Coin",
      "decimals": 6,
      "logoUri": "https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v/logo.png",
      "price": {
        "usd": "0.999701032204846900",
        "source": "jupiter",
        "fetchedAt": "2026-05-21T10:06:53.095599Z"
      }
    },
    "first_seen_at": "2026-05-21T10:02:09.917733Z",
    "last_seen_at": "2026-05-21T10:03:09.454266Z"
  };
}

describe("PoolSchema", () => {
  it("accepts a complete valid pool", () => {
    const parsed = PoolSchema.parse(validPool());
    expect(parsed.pool_address).toBe("BhVFo9nCA9X45yUUa7QgwUkR4mZcAop2kytSNhmQiS4C");
    expect(parsed.protocol).toBe("meteora_damm_v2");
  });

  it("accepts RFC3339 with a numeric timezone offset", () => {
    const parsed = PoolSchema.parse({
      ...validPool(),
      first_seen_at: "2026-05-01T08:30:00+02:00",
    });
    expect(parsed.first_seen_at).toBe("2026-05-01T08:30:00+02:00");
  });

  it("rejects an empty pool_address", () => {
    expect(() =>
      PoolSchema.parse({ ...validPool(), pool_address: "" }),
    ).toThrow();
  });

  it("rejects a non-RFC3339 timestamp", () => {
    expect(() =>
      PoolSchema.parse({ ...validPool(), first_seen_at: "yesterday" }),
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