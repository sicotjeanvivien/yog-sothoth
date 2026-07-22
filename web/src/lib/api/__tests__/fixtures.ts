/**
 * Test fixtures shared across the lib/api test suites.
 *
 * Builders rather than constants so each test gets a fresh, mutable
 * object — avoids cross-test pollution from accidental mutations.
 */

/** Build a representative valid pool payload. */
export function validPool() {
  return {
    poolAddress: "8Pm2kZpnxD3hoMmt4bjStX2Pw2Z9abpbHzZxMPqxPmie",
    protocol: "meteora_damm_v2",
    tokenA: {
      mint: "So11111111111111111111111111111111111111112",
      symbol: "SOL",
      name: "Wrapped SOL",
      decimals: 9,
      logoUri:
        "https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/So11111111111111111111111111111111111111112/logo.png",
      price: {
        usd: "85.819299811880730000",
        provider: "jupiter",
        fetchedAt: "2026-05-25T12:17:17.479657Z",
      },
    },
    tokenB: {
      mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
      symbol: "USDC",
      name: "USD Coin",
      decimals: 6,
      logoUri:
        "https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v/logo.png",
      price: {
        usd: "0.999668653937465800",
        provider: "jupiter",
        fetchedAt: "2026-05-25T12:17:17.479657Z",
      },
    },
    feeBps: "25",
    protocolFeePercent: 20,
    partnerFeePercent: 0,
    referralFeePercent: 20,
    baseFeeKind: "constant",
    hasDynamicFee: false,
    tvlUsd: "1332007.7148736200400326721044",
    volume24hUsd: "47964.973514780605664520660399",
    fees24hUsd: "119.912433786951514161301650",
    protocolFees24hUsd: "23.982486757390302832260330",
    lpFees24hUsd: "95.929947029561211329041320",
    effectiveFeeBps: "25",
    signals24h: [
      {
        severity: "warning",
        detector: "flow_imbalance",
        triggeredAt: "2026-05-25T11:47:02.000000Z",
      },
    ],
    firstSeenAt: "2026-05-21T10:01:35.084596Z",
    lastSeenAt: "2026-05-25T12:14:01.715170Z",
  };
}

/** Build a representative valid pool history bucket. */
export function validPoolHistoryBucket() {
  return {
    bucket: "2026-06-15T11:00:00Z",
    volumeUsd: "16070.42",
    feesUsd: "160.70",
    protocolFeesUsd: "30.45",
    lpFeesUsd: "130.25",
    effectiveFeeBps: "100",
    liquidityAddedUsd: null,
    liquidityRemovedUsd: null,
    feesClaimedUsd: null,
    rewardsClaimedUsd: null,
    swapCount: 96,
  };
}

/**
 * Build a representative valid pools page with bidirectional
 * pagination metadata. Defaults represent a "first page with more
 * data after" state — overrides let each test exercise other shapes
 * (terminal, single-page, middle, etc.).
 */
export function validPoolsPage(
  overrides: Partial<{
    items: ReturnType<typeof validPool>[];
    nextCursor: string | null;
    prevCursor: string | null;
    isFirst: boolean;
    isLast: boolean;
  }> = {},
) {
  return {
    items: [validPool()],
    nextCursor: "next-cursor-opaque",
    prevCursor: null,
    isFirst: true,
    isLast: false,
    ...overrides,
  };
}