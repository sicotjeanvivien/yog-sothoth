/**
 * Pool spot price — display formatting.
 *
 * The spot price itself is derived **server-side** from the pool's
 * `sqrt_price` and exposed as `PoolCurrentStateResponse.spotPriceAInB`
 * (DAMM v2 is concentrated liquidity, so the reserve ratio is *not* the
 * spot price — see `yog_core::amm::damm_v2::sqrt_price_to_price_a_in_b`).
 * The frontend only formats that value for display.
 */

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
