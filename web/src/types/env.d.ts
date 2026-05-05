// Local augmentation of `process.env` typing.
//
// TypeScript ships `process.env` as `Record<string, string | undefined>`,
// which means accessing a property with the dotted form
// `process.env.X` is flagged by the strict `noPropertyAccessFromIndex-
// Signature` rule.
//
// Switching to bracket notation (`process.env["X"]`) would silence
// the rule but break Next.js's build-time inlining of `NEXT_PUBLIC_*`
// values: the inliner only recognizes literal property accesses
// (https://nextjs.org/docs/app/api-reference/config/env#environment-variables).
// Inlining is what makes feature flags reachable from Client
// Components, so we cannot give it up.
//
// The ambient declaration below promotes every known env var to a
// real declared property of `ProcessEnv`. The dotted access then
// targets a real property — not the index signature inherited from
// `@types/node` — and the rule no longer applies.
//
// IMPORTANT — `string | undefined`, not `string?`
// ------------------------------------------------
// Each property is declared as a *required* property typed as
// `string | undefined`, not as an optional `?: string`. With
// `exactOptionalPropertyTypes: true`, an optional property is treated
// as semantically distinct from a required `string | undefined`
// property, and TypeScript falls back to the inherited index
// signature for the lookup. Required `string | undefined` matches
// the runtime semantics ("the property exists, its value may be
// undefined") and reliably overrides the index signature.
//
// Side benefit: editors gain autocompletion and typo detection on
// `process.env.NEXT_PUBLIC_FEATURE_*` across the whole project.
//
// Keep this list in sync with:
//   - the `RAW_VALUES` map in `src/config/features.ts`
//   - the variables documented in `.env.example`

declare global {
  namespace NodeJS {
    interface ProcessEnv {
      // ---- Database ----
      DATABASE_URL: string | undefined;

      // ---- Feature flags — pool data ----
      NEXT_PUBLIC_FEATURE_POOLS_LIST: string | undefined;
      NEXT_PUBLIC_FEATURE_POOL_DETAIL: string | undefined;
      NEXT_PUBLIC_FEATURE_POOL_PRICE_IMBALANCE: string | undefined;
      NEXT_PUBLIC_FEATURE_TRANSACTION_FEED: string | undefined;

      // ---- Feature flags — aggregated metrics ----
      NEXT_PUBLIC_FEATURE_TVL_TOTAL: string | undefined;
      NEXT_PUBLIC_FEATURE_VOLUME_24H: string | undefined;
      NEXT_PUBLIC_FEATURE_FEES_24H: string | undefined;
      NEXT_PUBLIC_FEATURE_TVL_CHART: string | undefined;
      NEXT_PUBLIC_FEATURE_PAIR_BREAKDOWN: string | undefined;
      NEXT_PUBLIC_FEATURE_KEY_METRICS: string | undefined;

      // ---- Feature flags — visualizations ----
      NEXT_PUBLIC_FEATURE_LIQUIDITY_MAP: string | undefined;
      NEXT_PUBLIC_FEATURE_LIQUIDITY_HEATMAP: string | undefined;
      NEXT_PUBLIC_FEATURE_LIVE_STATUS_BAR: string | undefined;

      // ---- Feature flags — scoring / Signal Engine ----
      NEXT_PUBLIC_FEATURE_LIQUIDITY_HEALTH_SCORE: string | undefined;
      NEXT_PUBLIC_FEATURE_ALERTS_PANEL: string | undefined;
      NEXT_PUBLIC_FEATURE_SIGNALS_FEED: string | undefined;
    }
  }
}

// `export {}` is intentionally omitted: it would turn this file into
// a module, and module-scoped `declare` blocks do not augment the
// global scope unless they sit inside `declare global`. The file
// stays a script (no imports/exports) and the `declare global`
// wrapper above makes the intent explicit.

export {};