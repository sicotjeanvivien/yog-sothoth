/**
 * A raw on-chain amount in units of its token: `formatTokenAmount("1480353083770", 9)` is
 * `"1.48K"`. ⚠️ `null` without decimals: a raw amount shown as units would be off by 10^decimals.
 * en-US forced, as in `format-count`.
 */

const COMPACT_FORMATTER = new Intl.NumberFormat("en-US", {
  notation: "compact",
  maximumFractionDigits: 2,
});

/** Under one unit, compact rounds to 0: three significant digits keep it readable. */
const SMALL_FORMATTER = new Intl.NumberFormat("en-US", { maximumSignificantDigits: 3 });

export function formatTokenAmount(raw: string, decimals: number | null): string | null {
  if (decimals === null || !/^\d+$/.test(raw)) {
    return null;
  }
  const units = BigInt(raw);
  const scale = 10n ** BigInt(decimals);
  const value = Number(units / scale) + Number(units % scale) / Number(scale);
  if (!Number.isFinite(value)) {
    return null;
  }
  return value > 0 && value < 1 ? SMALL_FORMATTER.format(value) : COMPACT_FORMATTER.format(value);
}
