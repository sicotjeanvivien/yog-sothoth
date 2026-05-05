import type { ReactNode } from "react";
import { isFeatureEnabled, type FeatureName } from "@/config/features";

type FeatureGateProps = {
  /** Name of the feature flag to evaluate. */
  flag: FeatureName;
  /** Content rendered when the flag is enabled. */
  children: ReactNode;
  /**
   * Optional content rendered when the flag is disabled. Defaults
   * to `null` so disabled widgets simply do not appear in the DOM.
   * Use this slot for explicit "coming soon" placeholders if needed.
   */
  fallback?: ReactNode;
};

/**
 * Conditionally render children based on a feature flag.
 *
 * Server Component by default — no `'use client'` directive — so
 * disabled widgets never ship code to the browser. The decision is
 * made at render time using values inlined at build time, which
 * keeps the runtime cost effectively zero.
 *
 * @example
 *   <FeatureGate flag="tvlTotal">
 *     <TvlTotalCard />
 *   </FeatureGate>
 *
 * @example
 *   <FeatureGate flag="liquidityHealthScore" fallback={<ComingSoon />}>
 *     <LiquidityHealthCard />
 *   </FeatureGate>
 */
export function FeatureGate({
  flag,
  children,
  fallback = null,
}: FeatureGateProps): ReactNode {
  return isFeatureEnabled(flag) ? children : fallback;
}