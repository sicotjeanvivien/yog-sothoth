/**
 * Format helpers for USD amounts coming from the API as strings.
 *
 * `formatUsdCompact`
 *   "845200000.50" → "$845.2M"
 *   "1280000.00"   → "$1.28M"
 *   "750.00"       → "$750"
 *   null / NaN     → "—"
 *
 * Not used in the current commit (TVL and 24h volume have yet to be
 * exposed by the API); kept here in advance because the format
 * convention matters and we want a single place to change it.
 *
 * Compact notation matches the convention used across DeFi
 * dashboards. Locale is forced to en-US so the decimal separator
 * stays a dot regardless of the visitor's locale.
 */

const COMPACT_FORMATTER = new Intl.NumberFormat("en-US", {
  notation: "compact",
  maximumFractionDigits: 2,
  style: "currency",
  currency: "USD",
});

const FORMATTER = new Intl.NumberFormat("en-US", {
  notation: "standard",
  maximumFractionDigits: 2,
  style: "currency",
  currency: "USD",
});

const EMPTY = "—";

export function formatUsdCompact(value: string | null | undefined): string {
  if (value === null || value === undefined) {
    return EMPTY;
  }
  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed)) {
    return EMPTY;
  }
  return COMPACT_FORMATTER.format(parsed);
}

export function formatUsd(value: string | null | undefined): string {
  if (value === null || value === undefined) {
    return EMPTY;
  }
  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed)) {
    return EMPTY;
  }
  return FORMATTER.format(parsed);
}