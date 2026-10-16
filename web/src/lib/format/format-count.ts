/**
 * A count in full, `12500` → `"12,500"`, null → "—". ⚠️ en-US forced, so the separator stays a
 * comma, as in `format-usd`.
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
