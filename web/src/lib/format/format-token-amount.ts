/**
 * A native-unit amount with its symbol: `("3450000000", 9, "SOL")` → `"3.45 SOL"`, compact from
 * 1000, `"< 0.000001"` below. The digit string survives the wire; the float is display only.
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