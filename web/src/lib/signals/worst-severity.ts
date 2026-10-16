/**
 * The worst severity of a set of signals: info < warning < critical, the Rust `Severity`
 * enum's order, in one place.
 */

import type { SeverityResponse } from "@/lib/api/schema/zod.gen";

const SEVERITY_RANK: Record<SeverityResponse, number> = {
  info: 0,
  warning: 1,
  critical: 2,
};

/** `null` when the list is empty — nothing to indicate. */
export function worstSeverity(
  signals: readonly { severity: SeverityResponse }[],
): SeverityResponse | null {
  let worst: SeverityResponse | null = null;
  for (const { severity } of signals) {
    if (worst === null || SEVERITY_RANK[severity] > SEVERITY_RANK[worst]) {
      worst = severity;
    }
  }
  return worst;
}
