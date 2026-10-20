/**
 * A ratio as a localized percentage or multiple, for display only. A string that does not
 * parse is returned as is, never "NaN %".
 */

/** "0.0523" → "+5.2 %" — sign always shown except on zero. */
export function formatSignedPercent(ratio: string, locale: string): string {
  return formatRatio(ratio, locale, "exceptZero");
}

/** "0.0500" → "5 %" — magnitude only (thresholds, leans). */
export function formatPercent(ratio: string, locale: string): string {
  return formatRatio(ratio, locale, "never");
}

/** 0.6812 → "× 0.68": how many times a base was turned over. */
export function formatTimes(ratio: number, locale: string): string {
  return `× ${new Intl.NumberFormat(locale, {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(ratio)}`;
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
