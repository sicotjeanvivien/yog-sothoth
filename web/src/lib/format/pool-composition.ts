/**
 * The share in [0, 1] of each side of a pool, from the USD values of
 * `/latest-state`. `null` when either is missing: a one-sided composition
 * would mislead, so the card is not rendered.
 */

export type PoolComposition = {
  valueAUsd: number;
  valueBUsd: number;
  shareA: number; // in [0, 1]
  shareB: number; // in [0, 1]
};

export type CompositionInput = {
  /** USD value of each side, a decimal string. Null when not valued. */
  valueAUsd: string | null;
  valueBUsd: string | null;
};

export function computePoolComposition(
  input: CompositionInput,
): PoolComposition | null {
  if (input.valueAUsd === null || input.valueBUsd === null) {
    return null;
  }

  const valueAUsd = Number.parseFloat(input.valueAUsd);
  const valueBUsd = Number.parseFloat(input.valueBUsd);

  if (!Number.isFinite(valueAUsd) || !Number.isFinite(valueBUsd)) {
    return null;
  }

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