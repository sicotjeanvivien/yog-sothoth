/**
 * Tests for the wire schemas.
 *
 * These tests act as the executable counterpart to the comments in
 * `schemas.ts`: if the Rust side drifts from the shape captured here,
 * the suite fails fast with a precise zod issue.
 */

import { describe, expect, it } from "vitest";
import { PoolSchema } from "../schema/pool";
import { PoolsPageSchema, SignalsPageSchema } from "../schema/page";
import { PoolHistorySchema } from "../schema/pool-history";
import { ApiErrorBodySchema } from "../schema/api-error-body";
import { SignalSchema } from "../schema/signal";
import { StatsSchema } from "../schema/stats";
import { validPoolsPage, validPoolHistoryBucket } from "./fixtures";


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
    "protocolFeePercent": 20,
    "partnerFeePercent": 0,
    "referralFeePercent": 20,
    "tvlUsd": "1332007.7148736200400326721044",
    "volume24hUsd": "47964.973514780605664520660399",
    "fees24hUsd": "119.912433786951514161301650",
    "protocolFees24hUsd": "23.982486757390302832260330",
    "lpFees24hUsd": "95.929947029561211329041320",
    "effectiveFeeBps": "25",
    "signals24h": [
      {
        "severity": "warning",
        "detector": "flow_imbalance",
        "triggeredAt": "2026-05-25T11:47:02.000000Z"
      }
    ],
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

  it("accepts an empty signals24h (quiet pool)", () => {
    const parsed = PoolSchema.parse({ ...validPool(), signals24h: [] });
    expect(parsed.signals24h).toEqual([]);
  });

  it("rejects a signals24h entry with an unknown severity", () => {
    expect(() =>
      PoolSchema.parse({
        ...validPool(),
        signals24h: [
          {
            severity: "catastrophic",
            detector: "flow_imbalance",
            triggeredAt: "2026-05-25T11:47:02.000000Z",
          },
        ],
      }),
    ).toThrow();
  });

  it("accepts a null feeBps (pool seen before its InitializePool)", () => {
    const parsed = PoolSchema.parse({ ...validPool(), feeBps: null });
    expect(parsed.feeBps).toBeNull();
  });

  it("accepts null fee-split percents (pool account not resolved yet)", () => {
    const parsed = PoolSchema.parse({
      ...validPool(),
      protocolFeePercent: null,
      partnerFeePercent: null,
      referralFeePercent: null,
    });
    expect(parsed.protocolFeePercent).toBeNull();
  });

  it("rejects a fee-split percent out of the 0..=100 range", () => {
    expect(() =>
      PoolSchema.parse({ ...validPool(), protocolFeePercent: 101 }),
    ).toThrow();
  });

  it("accepts null fee analytics (no priced swap in the window)", () => {
    const parsed = PoolSchema.parse({
      ...validPool(),
      fees24hUsd: null,
      protocolFees24hUsd: null,
      lpFees24hUsd: null,
      effectiveFeeBps: null,
    });
    expect(parsed.fees24hUsd).toBeNull();
    expect(parsed.effectiveFeeBps).toBeNull();
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

describe("PoolHistorySchema", () => {
  it("accepts an array of valid buckets", () => {
    const parsed = PoolHistorySchema.parse([
      validPoolHistoryBucket(),
      validPoolHistoryBucket(),
    ]);
    expect(parsed).toHaveLength(2);
    expect(parsed[0]?.feesUsd).toBe("160.70");
    expect(parsed[0]?.swapCount).toBe(96);
  });

  it("accepts an empty series", () => {
    expect(PoolHistorySchema.parse([])).toHaveLength(0);
  });

  it("accepts null USD metrics (a bucket with one source of activity)", () => {
    const parsed = PoolHistorySchema.parse([
      { ...validPoolHistoryBucket(), feesUsd: null, effectiveFeeBps: null },
    ]);
    expect(parsed[0]?.feesUsd).toBeNull();
  });

  it("rejects a non-RFC3339 bucket timestamp", () => {
    expect(() =>
      PoolHistorySchema.parse([{ ...validPoolHistoryBucket(), bucket: "nope" }]),
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

describe("StatsSchema", () => {
  const validStats = () => ({
    totalTvlUsd: "10427935.81",
    poolsPriced: 348,
    volume24hUsd: "508193.05",
    fees24hUsd: "391.03",
    poolsObserved: 359,
    poolsDiscovered24h: 52,
  });

  it("accepts a complete valid payload", () => {
    const parsed = StatsSchema.parse(validStats());
    expect(parsed.totalTvlUsd).toBe("10427935.81");
    expect(parsed.poolsObserved).toBe(359);
    expect(parsed.poolsDiscovered24h).toBe(52);
  });

  it("accepts null USD aggregates (empty universe / no activity)", () => {
    const parsed = StatsSchema.parse({
      ...validStats(),
      totalTvlUsd: null,
      volume24hUsd: null,
      fees24hUsd: null,
      poolsPriced: 0,
    });
    expect(parsed.totalTvlUsd).toBeNull();
    expect(parsed.volume24hUsd).toBeNull();
    expect(parsed.poolsPriced).toBe(0);
  });

  it("rejects a USD aggregate sent as a JS number (precision contract)", () => {
    expect(() =>
      StatsSchema.parse({ ...validStats(), totalTvlUsd: 10427935.81 }),
    ).toThrow();
  });

  it("rejects a negative or fractional count", () => {
    expect(() =>
      StatsSchema.parse({ ...validStats(), poolsObserved: -1 }),
    ).toThrow();
    expect(() =>
      StatsSchema.parse({ ...validStats(), poolsDiscovered24h: 1.5 }),
    ).toThrow();
  });

  it("rejects a missing field", () => {
    const { poolsObserved, ...rest } = validStats();
    void poolsObserved;
    expect(() => StatsSchema.parse(rest)).toThrow();
  });
});
// A representative valid signal payload, copied from a real yog-api
// response (both the list items and the SSE events carry this shape).
function validSignal() {
  return {
    id: 739,
    detector: "price_oracle_deviation",
    protocol: "meteora_damm_v2",
    poolAddress: "5NMi3SSebB7MyP17Sf5SwXh6nTnPPp1Ctb1G45NnRKuZ",
    tokenA: {
      mint: "So11111111111111111111111111111111111111112",
      symbol: "SOL",
      name: "Wrapped SOL",
      decimals: 9,
      logoUri: null,
      price: {
        usd: "85.819299811880730000",
        provider: "jupiter",
        fetchedAt: "2026-05-25T12:17:17.479657Z",
      },
    },
    tokenB: {
      mint: null,
      symbol: null,
      name: null,
      decimals: 0,
      logoUri: null,
      price: null,
    },
    severity: "critical",
    value: "1.1059558665865029280736523078",
    threshold: "0.0500",
    message: "spot price deviates 1.1060 from oracle (spot 0.000000582488, oracle 0.000000276591)",
    triggeredAt: "2026-07-02T10:03:31.056847Z",
  };
}

describe("SignalSchema", () => {
  it("accepts a real payload", () => {
    expect(() => SignalSchema.parse(validSignal())).not.toThrow();
  });

  it("accepts a negative value (a deviation can be below the oracle)", () => {
    const parsed = SignalSchema.parse({ ...validSignal(), value: "-0.2157" });
    expect(parsed.value).toBe("-0.2157");
  });

  it("accepts null threshold and message", () => {
    const parsed = SignalSchema.parse({
      ...validSignal(),
      threshold: null,
      message: null,
    });
    expect(parsed.threshold).toBeNull();
    expect(parsed.message).toBeNull();
  });

  it("rejects an unknown severity (closed set)", () => {
    expect(() =>
      SignalSchema.parse({ ...validSignal(), severity: "panic" }),
    ).toThrow();
  });

  it("rejects a value sent as a JS number (precision contract)", () => {
    expect(() =>
      SignalSchema.parse({ ...validSignal(), value: 1.1059 }),
    ).toThrow();
  });

  it("rejects a fractional id", () => {
    expect(() => SignalSchema.parse({ ...validSignal(), id: 1.5 })).toThrow();
  });
});

describe("SignalsPageSchema", () => {
  it("accepts a page envelope of signals", () => {
    expect(() =>
      SignalsPageSchema.parse({
        items: [validSignal()],
        nextCursor: "opaque",
        prevCursor: null,
        isFirst: true,
        isLast: false,
      }),
    ).not.toThrow();
  });
});
