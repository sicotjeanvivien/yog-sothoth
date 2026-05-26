/**
 * Pool composition — pure math helpers.
 *
 * Computes the USD value of each side of a pool from its reserves
 * (native units, integer string) and the latest price for that
 * side, then derives the share in [0, 1].
 *
 * Kept in `lib/format/` (or `lib/pool/` if you prefer; the file is
 * pure and has no UI dependency) so the SVG donut component can
 * import a single deterministic function and stay small.
 *
 * Returns `null` when any required input is missing — the caller
 * should treat that as "don't render the composition card". A
 * partial composition (one side priced, the other not) would be
 * misleading rather than useful.
 */

export type PoolComposition = {
  valueAUsd: number;
  valueBUsd: number;
  shareA: number; // in [0, 1]
  shareB: number; // in [0, 1]
};

export type CompositionInput = {
  /** Reserve in native units, integer string from the API. */
  reserveA: string;
  reserveB: string;
  /** Decimals of each token. */
  decimalsA: number;
  decimalsB: number;
  /** Latest USD price as a string ("BigDecimal" on the wire). Null when not priced. */
  priceAUsd: string | null;
  priceBUsd: string | null;
};

export function computePoolComposition(
  input: CompositionInput,
): PoolComposition | null {
  if (input.priceAUsd === null || input.priceBUsd === null) {
    return null;
  }

  const reserveA = Number.parseFloat(input.reserveA);
  const reserveB = Number.parseFloat(input.reserveB);
  const priceA = Number.parseFloat(input.priceAUsd);
  const priceB = Number.parseFloat(input.priceBUsd);

  if (
    !Number.isFinite(reserveA) ||
    !Number.isFinite(reserveB) ||
    !Number.isFinite(priceA) ||
    !Number.isFinite(priceB)
  ) {
    return null;
  }

  const valueAUsd = (reserveA / 10 ** input.decimalsA) * priceA;
  const valueBUsd = (reserveB / 10 ** input.decimalsB) * priceB;
  const total = valueAUsd + valueBUsd;

  if (total <= 0) {
    return null;
  }

  return {
    valueAUsd,
    valueBUsd,
    shareA: valueAUsd / total,
    shareB: valueBUsd / total,
  };
}

/**
 * Convert a share in [0, 1] to the (x, y) coordinates of a point
 * on a unit circle starting from the top (12 o'clock) and going
 * clockwise.
 *
 * Used by the donut renderer to build SVG arc paths.
 */
export function shareToCircleCoords(share: number): { x: number; y: number } {
  // Top of the circle = -π/2 in standard math notation; going
  // clockwise means adding share * 2π.
  const angle = -Math.PI / 2 + share * 2 * Math.PI;
  return {
    x: Math.cos(angle),
    y: Math.sin(angle),
  };
}