/**
 * Worst severity of a set of signals — drives the color/shape of the
 * pools-list signal indicator. Pure and shared so the ordering
 * (info < warning < critical, mirroring the Rust `Severity` enum's
 * escalation order) lives in exactly one place.
 */

import type { Severity } from "@/lib/api/schema/signal";

const SEVERITY_RANK: Record<Severity, number> = {
  info: 0,
  warning: 1,
  critical: 2,
};

/** `null` when the list is empty — nothing to indicate. */
export function worstSeverity(
  signals: readonly { severity: Severity }[],
): Severity | null {
  let worst: Severity | null = null;
  for (const { severity } of signals) {
    if (worst === null || SEVERITY_RANK[severity] > SEVERITY_RANK[worst]) {
      worst = severity;
    }
  }
  return worst;
}
