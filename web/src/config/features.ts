// Feature flag system for the Yog-Sothoth dashboard.
//
// Flags are read from `NEXT_PUBLIC_FEATURE_*` environment variables.
// Because Next.js inlines `NEXT_PUBLIC_*` values at build time on the
// client, flipping a flag in production requires a rebuild and a
// redeploy. This is a build-time toggle, not a runtime toggle.
//
// All flags default to `false`. Only the literal string "true" (case
// sensitive) is treated as on; anything else, including missing
// values, "1", "yes", "TRUE", etc., is treated as off. The strict
// parser keeps configuration mistakes safe-by-default.
//
// IMPORTANT — client-side inlining requirement
// --------------------------------------------
// Next.js inlines `process.env.NEXT_PUBLIC_*` at build time only when
// the access is a *literal* property reference (e.g.
// `process.env.NEXT_PUBLIC_FEATURE_POOLS_LIST`). Dynamic accesses
// like `process.env[name]` are not inlined and return `undefined`
// on the client. The `RAW_VALUES` map below intentionally lists each
// env var by hand so that every flag is reachable from Client
// Components.

/**
 * Status describing why a flag is on or off in v0.1.
 *
 * - `available-v0.1`: data is in TimescaleDB today, widget can ship
 * - `degraded-v0.1`: data is available but the rendering path uses
 *   a less-than-ideal mechanism (e.g. polling instead of WebSocket)
 * - `pending-aggregate`: needs a TimescaleDB continuous aggregate
 *   that is not created yet
 * - `pending-signals`: depends on the v0.2 Signal Engine
 * - `pending-design`: scoring or formula not yet defined
 * - `pending-visual`: bespoke visualization not in scope for v0.1
 */
export type FeatureStatus =
  | "available-v0.1"
  | "degraded-v0.1"
  | "pending-aggregate"
  | "pending-signals"
  | "pending-design"
  | "pending-visual";

/**
 * Single source of truth for every flag the app knows about. Adding
 * a flag here makes it both type-safe (autocompletion of `FeatureName`)
 * and discoverable (one file lists them all with their rationale).
 */
export const FEATURE_REGISTRY = {
  // ---- Pool data ----
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

  // ---- Aggregated metrics (need continuous aggregates) ----
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

  // ---- Visualizations not in scope for v0.1 ----
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

  // ---- Scoring / Signal Engine (v0.2) ----
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

/** Union of every known flag name, derived from the registry. */
export type FeatureName = keyof typeof FEATURE_REGISTRY;

/**
 * Literal env-var accesses. Each entry MUST use the dotted form
 * `process.env.NEXT_PUBLIC_FEATURE_X` so that Next.js inlines the
 * value into the client bundle at build time.
 *
 * Keep the keys in sync with `FEATURE_REGISTRY`. The exhaustive type
 * `Record<FeatureName, ...>` makes TypeScript fail the build if a
 * flag is added to the registry but missing here.
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

/**
 * Strict boolean parser: only the literal string "true" is treated
 * as enabled. Any other value, including undefined, returns false.
 *
 * Intentionally narrow to avoid the classic pitfall of "I typed
 * `True` with a capital T and the flag silently stayed off".
 */
export function parseStrictBoolean(raw: string | undefined): boolean {
  return raw === "true";
}

/**
 * Resolve the effective state of a feature flag.
 *
 * Resolution order:
 *   1. The matching `NEXT_PUBLIC_FEATURE_*` env var if it is set
 *      (any non-undefined value is considered an explicit override,
 *      even an empty string, which evaluates to `false`).
 *   2. The `defaultEnabled` value declared in the registry.
 *
 * Safe to call from both Server and Client Components.
 */
export function isFeatureEnabled(flag: FeatureName): boolean {
  const raw = RAW_VALUES[flag];
  if (raw !== undefined) {
    return parseStrictBoolean(raw);
  }
  return FEATURE_REGISTRY[flag].defaultEnabled;
}