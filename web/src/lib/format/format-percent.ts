/**
 * Ratio → localized percentage, for detector metrics.
 *
 * Signal `value` / `threshold` arrive as exact decimal strings where
 * both current detectors express a *ratio* (0.05 = 5%). Display-only:
 * parsing through `Number` is fine here, the exact string stays in the
 * payload for anything that needs precision.
 *
 * A string that doesn't parse (future detector with non-numeric
 * semantics, drifted payload) is returned as-is rather than rendered
 * "NaN %".
 */

/** "0.0523" → "+5.2 %" — sign always shown except on zero. */
export function formatSignedPercent(ratio: string, locale: string): string {
  return formatRatio(ratio, locale, "exceptZero");
}

/** "0.0500" → "5 %" — magnitude only (thresholds, leans). */
export function formatPercent(ratio: string, locale: string): string {
  return formatRatio(ratio, locale, "never");
}

function formatRatio(
  ratio: string,
  locale: string,
  signDisplay: "exceptZero" | "never",
): string {
  const value = Number(ratio);
  if (!Number.isFinite(value)) {
    return ratio;
  }
  const magnitude = signDisplay === "never" ? Math.abs(value) : value;
  return new Intl.NumberFormat(locale, {
    style: "percent",
    maximumFractionDigits: 1,
    signDisplay,
  }).format(magnitude);
}
