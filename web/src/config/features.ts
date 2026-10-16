// Feature flags, from `NEXT_PUBLIC_FEATURE_*`: inlined at build time, so flipping one needs a
// rebuild. Only the literal "true" turns a flag on.

/** Why a flag is on or off in v0.1. */
export type FeatureStatus =
  | "available-v0.1"
  | "degraded-v0.1"
  | "pending-aggregate"
  | "pending-signals"
  | "pending-design"
  | "pending-visual";

/** Every flag the app knows, with its rationale. */
export const FEATURE_REGISTRY = {
  poolsList: {
    description: "List of pools observed by the indexer.",
    status: "available-v0.1",
    defaultEnabled: true,
  },
  poolDetail: {
    description: "Pool detail page: mints, first_seen, last_seen.",
    status: "available-v0.1",
    defaultEnabled: true,
  },
  poolPriceImbalance: {
    description: "Current price and imbalance derived from latest reserves.",
    status: "available-v0.1",
    defaultEnabled: true,
  },
  transactionFeed: {
    description:
      "Live transaction feed (polling-based until WebSocket push arrives).",
    status: "degraded-v0.1",
    defaultEnabled: true,
  },

  tvlTotal: {
    description: "Global TVL across every observed pool.",
    status: "pending-aggregate",
    defaultEnabled: false,
  },
  volume24h: {
    description: "24h trading volume aggregate.",
    status: "pending-aggregate",
    defaultEnabled: false,
  },
  fees24h: {
    description: "24h fee revenue aggregate.",
    status: "pending-aggregate",
    defaultEnabled: false,
  },
  tvlChart: {
    description: "7-day TVL chart on the dashboard.",
    status: "pending-aggregate",
    defaultEnabled: false,
  },
  pairBreakdown: {
    description: "Donut breakdown of TVL by trading pair.",
    status: "pending-aggregate",
    defaultEnabled: false,
  },
  keyMetrics: {
    description: "Concentration, depth, slippage, volatility metrics block.",
    status: "pending-aggregate",
    defaultEnabled: false,
  },

  liquidityMap: {
    description: "Bespoke 'liquidity map' visualization of all pools.",
    status: "pending-visual",
    defaultEnabled: false,
  },
  liquidityHeatmap: {
    description: "24h liquidity change heatmap by token.",
    status: "pending-visual",
    defaultEnabled: false,
  },
  liveStatusBar: {
    description: "Top status bar: LIVE indicator, mainnet, slot, time.",
    status: "pending-visual",
    defaultEnabled: false,
  },

  liquidityHealthScore: {
    description: "Pool health score (0-100). Formula not yet defined.",
    status: "pending-design",
    defaultEnabled: false,
  },
  alertsPanel: {
    description: "Alerts panel on the dashboard.",
    status: "pending-signals",
    defaultEnabled: false,
  },
  signalsFeed: {
    description: "Feed of detected signals from the Signal Engine.",
    status: "pending-signals",
    defaultEnabled: false,
  },
} as const;

export type FeatureName = keyof typeof FEATURE_REGISTRY;

/**
 * ⚠️ One literal `process.env.NEXT_PUBLIC_FEATURE_X` per flag: Next.js inlines only literal
 * accesses, and `process.env[name]` is `undefined` on the client.
 */
const RAW_VALUES: Record<FeatureName, string | undefined> = {
  poolsList: process.env.NEXT_PUBLIC_FEATURE_POOLS_LIST,
  poolDetail: process.env.NEXT_PUBLIC_FEATURE_POOL_DETAIL,
  poolPriceImbalance: process.env.NEXT_PUBLIC_FEATURE_POOL_PRICE_IMBALANCE,
  transactionFeed: process.env.NEXT_PUBLIC_FEATURE_TRANSACTION_FEED,
  tvlTotal: process.env.NEXT_PUBLIC_FEATURE_TVL_TOTAL,
  volume24h: process.env.NEXT_PUBLIC_FEATURE_VOLUME_24H,
  fees24h: process.env.NEXT_PUBLIC_FEATURE_FEES_24H,
  tvlChart: process.env.NEXT_PUBLIC_FEATURE_TVL_CHART,
  pairBreakdown: process.env.NEXT_PUBLIC_FEATURE_PAIR_BREAKDOWN,
  keyMetrics: process.env.NEXT_PUBLIC_FEATURE_KEY_METRICS,
  liquidityMap: process.env.NEXT_PUBLIC_FEATURE_LIQUIDITY_MAP,
  liquidityHeatmap: process.env.NEXT_PUBLIC_FEATURE_LIQUIDITY_HEATMAP,
  liveStatusBar: process.env.NEXT_PUBLIC_FEATURE_LIVE_STATUS_BAR,
  liquidityHealthScore:
    process.env.NEXT_PUBLIC_FEATURE_LIQUIDITY_HEALTH_SCORE,
  alertsPanel: process.env.NEXT_PUBLIC_FEATURE_ALERTS_PANEL,
  signalsFeed: process.env.NEXT_PUBLIC_FEATURE_SIGNALS_FEED,
};

/** Only the literal "true" is on: `True` or `1` stay off. */
export function parseStrictBoolean(raw: string | undefined): boolean {
  return raw === "true";
}

/** The env var when set — even empty, which is off — else the registry's `defaultEnabled`. */
export function isFeatureEnabled(flag: FeatureName): boolean {
  const raw = RAW_VALUES[flag];
  if (raw !== undefined) {
    return parseStrictBoolean(raw);
  }
  return FEATURE_REGISTRY[flag].defaultEnabled;
}