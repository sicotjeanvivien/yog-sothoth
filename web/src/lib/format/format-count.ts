/**
 * Format helper for plain integer counts coming from the API as numbers.
 *
 * `formatCount`
 *   359          → "359"
 *   12500        → "12,500"
 *   null / NaN   → "—"
 *
 * Standard notation with grouping separators (no compaction): counts on
 * the dashboard are small and read more clearly in full than as "12.5K".
 * Locale forced to en-US so the grouping separator stays a comma
 * regardless of the visitor's locale, matching `format-usd`.
 */

const FORMATTER = new Intl.NumberFormat("en-US", {
  notation: "standard",
  maximumFractionDigits: 0,
});

const EMPTY = "—";

export function formatCount(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return EMPTY;
  }
  return FORMATTER.format(value);
}
