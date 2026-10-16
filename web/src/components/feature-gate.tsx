import type { ReactNode } from "react";
import { isFeatureEnabled, type FeatureName } from "@/config/features";

type FeatureGateProps = {
  flag: FeatureName;
  children: ReactNode;
  /** Rendered when the flag is off; nothing by default. */
  fallback?: ReactNode;
};

/** Renders `children` when `flag` is on. A Server Component: a disabled widget ships no code. */
export function FeatureGate({
  flag,
  children,
  fallback = null,
}: FeatureGateProps): ReactNode {
  return isFeatureEnabled(flag) ? children : fallback;
}