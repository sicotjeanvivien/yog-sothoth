/**
 * Pool spot price — pure math helpers.
 *
 * Derives the pool's quoted exchange rate between its two tokens from
 * the latest reserves. This follows the project's stated convention:
 * price is computed at query time from reserves, not from `sqrt_price`
 * (see the `MeteoraDammV2SwapEvent` domain doc — "no derived analytics
 * […] computed at query time from the reserves recorded here").
 *
 *   priceAInB = (reserveB / 10^decB) / (reserveA / 10^decA)
 *             = how many units of token B one unit of token A is worth
 *   priceBInA = its reciprocal
 *
 * Returns `null` when a reserve is missing / non-finite or when either
 * side is empty — a zero reserve has no defined price. The caller then
 * omits the price card (factual or absent, never fake).
 */

export type PoolPrice = {
  /** Units of B per 1 unit of A. */
  priceAInB: number;
  /** Units of A per 1 unit of B. */
  priceBInA: number;
};

export type PoolPriceInput = {
  /** Reserve in native units, integer string from the API. */
  reserveA: string;
  reserveB: string;
  /** Decimals of each token. */
  decimalsA: number;
  decimalsB: number;
};

export function computePoolPrice(input: PoolPriceInput): PoolPrice | null {
  const reserveA = Number.parseFloat(input.reserveA);
  const reserveB = Number.parseFloat(input.reserveB);

  if (!Number.isFinite(reserveA) || !Number.isFinite(reserveB)) {
    return null;
  }

  const humanA = reserveA / 10 ** input.decimalsA;
  const humanB = reserveB / 10 ** input.decimalsB;

  if (humanA <= 0 || humanB <= 0) {
    return null;
  }

  return {
    priceAInB: humanB / humanA,
    priceBInA: humanA / humanB,
  };
}

// ── Formatting ────────────────────────────────────────────────────────

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
 * Format a price (a plain ratio, already decimal-adjusted) with a
 * precision strategy picked from its order of magnitude — mirrors the
 * tiers used by `formatTokenAmount`:
 *
 *   ≥ 1000      → compact ("1.2M")
 *   ≥ 1         → 2 fraction digits ("152.34")
 *   ≥ 0.00000001 → up to 4 significant digits ("0.006572")
 *   <           → "< 0.00000001"
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
