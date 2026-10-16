// Declares env vars on `ProcessEnv`, so the dotted `process.env.X` passes
// `noPropertyAccessFromIndexSignature`. Kept in step with `RAW_VALUES`.

declare global {
  namespace NodeJS {
    interface ProcessEnv {
      YOG_API_INTERNAL_URL?: string;
      YOG_API_TIMEOUT_MS?: string;

      DATABASE_URL?: string;

      NEXT_PUBLIC_FEATURE_POOLS_LIST?: string;
      NEXT_PUBLIC_FEATURE_POOL_DETAIL?: string;
      NEXT_PUBLIC_FEATURE_POOL_PRICE_IMBALANCE?: string;
      NEXT_PUBLIC_FEATURE_TRANSACTION_FEED?: string;

      NEXT_PUBLIC_FEATURE_TVL_TOTAL?: string;
      NEXT_PUBLIC_FEATURE_VOLUME_24H?: string;
      NEXT_PUBLIC_FEATURE_FEES_24H?: string;
      NEXT_PUBLIC_FEATURE_TVL_CHART?: string;
      NEXT_PUBLIC_FEATURE_PAIR_BREAKDOWN?: string;
      NEXT_PUBLIC_FEATURE_KEY_METRICS?: string;

      NEXT_PUBLIC_FEATURE_LIQUIDITY_MAP?: string;
      NEXT_PUBLIC_FEATURE_LIQUIDITY_HEATMAP?: string;
      NEXT_PUBLIC_FEATURE_LIVE_STATUS_BAR?: string;

      NEXT_PUBLIC_FEATURE_LIQUIDITY_HEALTH_SCORE?: string;
      NEXT_PUBLIC_FEATURE_ALERTS_PANEL?: string;
      NEXT_PUBLIC_FEATURE_SIGNALS_FEED?: string;
    }
  }
}

export { };