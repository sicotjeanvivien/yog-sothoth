/**
 * Formats the spot price the server derives from `sqrt_price`
 * (`PoolCurrentStateResponse.spotPriceAInB`): the reserve ratio is not the spot price of a
 * concentrated pool.
 */

const COMPACT_FORMATTER = new Intl.NumberFormat("en-US", {
  notation: "compact",
  maximumFractionDigits: 2,
});

const STANDARD_FORMATTER = new Intl.NumberFormat("en-US", {
  maximumFractionDigits: 2,
});

const SMALL_FORMATTER = new Intl.NumberFormat("en-US", {
  maximumSignificantDigits: 4,
});

const EMPTY = "—";

/**
 * A decimal-adjusted ratio, with the tiers of `formatTokenAmount`: compact from 1000, two digits
 * from 1, four significant digits below, `"< 0.00000001"` under that.
 */
export function formatPrice(value: number): string {
  if (!Number.isFinite(value) || value <= 0) {
    return EMPTY;
  }
  if (value >= 1000) {
    return COMPACT_FORMATTER.format(value);
  }
  if (value >= 1) {
    return STANDARD_FORMATTER.format(value);
  }
  if (value >= 1e-8) {
    return SMALL_FORMATTER.format(value);
  }
  return "< 0.00000001";
}
