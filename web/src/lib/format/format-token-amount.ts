/**
 * Format a token amount expressed in native units (the integer
 * value the chain emits) as a human-readable string with the
 * token's symbol appended.
 *
 *   formatTokenAmount("3450000000", 9, "SOL")     → "3.45 SOL"
 *   formatTokenAmount("512240000", 6, "USDC")     → "512.24 USDC"
 *   formatTokenAmount("1250000000000", 6, "USDC") → "1.25M USDC"
 *   formatTokenAmount("123", 9, "SOL")            → "0.000000123 SOL"
 *
 * The function picks a precision strategy from the order of
 * magnitude:
 *
 *   ≥ 1000     → compact notation ("1.25K", "3.45M")
 *   ≥ 1        → 2 fraction digits
 *   ≥ 0.000001 → up to 6 significant digits, no trailing zeros
 *   <          → "< 0.000001" (sub-micro amounts are noise)
 *
 * `amount` is a digit-only string: native u64 values arrive from the
 * API as strings to survive the JS 2^53 ceiling (a u64 atomic amount
 * can exceed it). For display we downcast to a float — the precision
 * beyond ~15 significant digits is irrelevant once formatted — but the
 * exact value is preserved on the wire for callers that need it.
 */

const COMPACT_FORMATTER = new Intl.NumberFormat("en-US", {
  notation: "compact",
  maximumFractionDigits: 2,
});

const STANDARD_FORMATTER = new Intl.NumberFormat("en-US", {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

const SMALL_AMOUNT_FORMATTER = new Intl.NumberFormat("en-US", {
  maximumSignificantDigits: 6,
});

const MIN_DISPLAYABLE = 1e-6;

export function formatTokenAmount(
  amount: string,
  decimals: number,
  symbol: string | null,
): string {
  const sym = symbol ?? "?";

  const raw = Number(amount);
  if (!Number.isFinite(raw) || raw < 0) {
    return `— ${sym}`;
  }

  const value = raw / 10 ** decimals;

  if (value === 0) {
    return `0 ${sym}`;
  }

  if (value >= 1000) {
    return `${COMPACT_FORMATTER.format(value)} ${sym}`;
  }

  if (value >= 1) {
    return `${STANDARD_FORMATTER.format(value)} ${sym}`;
  }

  if (value >= MIN_DISPLAYABLE) {
    return `${SMALL_AMOUNT_FORMATTER.format(value)} ${sym}`;
  }

  return `< 0.000001 ${sym}`;
}