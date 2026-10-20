/**
 * A count in full, `12500` → `"12,500"`, or compact, `3410000` → `"3.41M"`; null → "—".
 * ⚠️ en-US forced, so the separator stays a comma, as in `format-usd`.
 */

const FORMATTER = new Intl.NumberFormat("en-US", {
  notation: "standard",
  maximumFractionDigits: 0,
});

const COMPACT_FORMATTER = new Intl.NumberFormat("en-US", {
  notation: "compact",
  maximumFractionDigits: 2,
});

const RATE_FORMATTER = new Intl.NumberFormat("en-US", { maximumSignificantDigits: 2 });

const EMPTY = "—";

export function formatCount(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return EMPTY;
  }
  return FORMATTER.format(value);
}

export function formatCountCompact(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return EMPTY;
  }
  return COMPACT_FORMATTER.format(value);
}

/** A rate per second: whole from 10, two significant digits under it, so a slow pace is never "0". */
export function formatRate(rate: number): string {
  return rate < 10 ? RATE_FORMATTER.format(rate) : formatCount(Math.round(rate));
}
