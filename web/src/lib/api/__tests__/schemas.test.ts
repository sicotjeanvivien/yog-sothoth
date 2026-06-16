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
import { validPoolsPage } from "./fixtures";


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
        "provider": "jupiter",
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
        "provider": "jupiter",
        "fetchedAt": "2026-05-25T12:17:17.479657Z"
      }
    },
    "feeBps": "25",
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

  it("accepts a null feeBps (pool seen before its InitializePool)", () => {
    const parsed = PoolSchema.parse({ ...validPool(), feeBps: null });
    expect(parsed.feeBps).toBeNull();
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
  it("accepts a single-page result (no neighbours)", () => {
    const parsed = PoolsPageSchema.parse(
      validPoolsPage({
        items: [],
        nextCursor: null,
        prevCursor: null,
        isFirst: true,
        isLast: true,
      }),
    );
    expect(parsed.items).toHaveLength(0);
    expect(parsed.nextCursor).toBeNull();
    expect(parsed.prevCursor).toBeNull();
    expect(parsed.isFirst).toBe(true);
    expect(parsed.isLast).toBe(true);
  });

  it("accepts a first page with more data after", () => {
    const parsed = PoolsPageSchema.parse(validPoolsPage());
    expect(parsed.items).toHaveLength(1);
    expect(parsed.prevCursor).toBeNull();
    expect(parsed.nextCursor).not.toBeNull();
    expect(parsed.isFirst).toBe(true);
    expect(parsed.isLast).toBe(false);
  });

  it("accepts a middle page with neighbours on both sides", () => {
    const parsed = PoolsPageSchema.parse(
      validPoolsPage({
        items: [validPool(), validPool()],
        nextCursor: "next-x",
        prevCursor: "prev-y",
        isFirst: false,
        isLast: false,
      }),
    );
    expect(parsed.items).toHaveLength(2);
    expect(parsed.nextCursor).toBe("next-x");
    expect(parsed.prevCursor).toBe("prev-y");
    expect(parsed.isFirst).toBe(false);
    expect(parsed.isLast).toBe(false);
  });

  it("accepts a terminal page (last page reached)", () => {
    const parsed = PoolsPageSchema.parse(
      validPoolsPage({
        nextCursor: null,
        prevCursor: "prev-z",
        isFirst: false,
        isLast: true,
      }),
    );
    expect(parsed.nextCursor).toBeNull();
    expect(parsed.isLast).toBe(true);
  });

  it("rejects items that fail individual validation", () => {
    expect(() =>
      PoolsPageSchema.parse(
        validPoolsPage({
          items: [{ ...validPool(), poolAddress: "" }],
        }),
      ),
    ).toThrow();
  });

  it("rejects a missing nextCursor field", () => {
    const { nextCursor, ...rest } = validPoolsPage();
    void nextCursor;
    expect(() => PoolsPageSchema.parse(rest)).toThrow();
  });

  it("rejects a missing prevCursor field", () => {
    const { prevCursor, ...rest } = validPoolsPage();
    void prevCursor;
    expect(() => PoolsPageSchema.parse(rest)).toThrow();
  });

  it("rejects a missing isFirst flag", () => {
    const { isFirst, ...rest } = validPoolsPage();
    void isFirst;
    expect(() => PoolsPageSchema.parse(rest)).toThrow();
  });

  it("rejects a missing isLast flag", () => {
    const { isLast, ...rest } = validPoolsPage();
    void isLast;
    expect(() => PoolsPageSchema.parse(rest)).toThrow();
  });

  it("rejects a non-boolean isFirst flag", () => {
    expect(() =>
      PoolsPageSchema.parse(validPoolsPage({ isFirst: "true" as unknown as boolean })),
    ).toThrow();
  });
});

describe("ApiErrorBodySchema", () => {
  function validProblem() {
    return {
      type: "about:blank",
      title: "Bad Request",
      status: 400,
      detail: "limit must be between 1 and 200",
    };
  }

  it("accepts a well-formed RFC 9457 problem", () => {
    const parsed = ApiErrorBodySchema.parse(validProblem());
    expect(parsed.type).toBe("about:blank");
    expect(parsed.title).toBe("Bad Request");
    expect(parsed.status).toBe(400);
    expect(parsed.detail).toBe("limit must be between 1 and 200");
  });

  it("accepts a future type URI", () => {
    // Forward compat: when yog-api moves off `about:blank`, the schema
    // must still parse.
    const parsed = ApiErrorBodySchema.parse({
      ...validProblem(),
      type: "https://api.yog-sothoth.fr/errors/invalid-cursor",
    });
    expect(parsed.type).toBe("https://api.yog-sothoth.fr/errors/invalid-cursor");
  });

  it("rejects a body missing required RFC 9457 fields", () => {
    expect(() => ApiErrorBodySchema.parse({ error: "old format" })).toThrow();
    expect(() => ApiErrorBodySchema.parse({ detail: "no other fields" })).toThrow();
  });

  it("rejects a non-numeric status", () => {
    expect(() =>
      ApiErrorBodySchema.parse({ ...validProblem(), status: "400" }),
    ).toThrow();
  });
});