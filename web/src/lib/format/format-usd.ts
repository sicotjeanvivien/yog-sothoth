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

/**
 * Format shares of a total so that what is DISPLAYED adds up.
 *
 * Rounding each share to the cent independently does not preserve a sum. The
 * fee split of a real pool sheet: `0.3800 + 0.0950 + 1.4250` is exactly
 * `1.9000`, but the three round to `$0.38 + $0.10 + $1.43 = $1.91` under a
 * total displayed as `$1.90`. One cent, on a card that invites the reader to
 * check the arithmetic — and the more shares, the more often.
 *
 * The fix is the standard largest-remainder allocation: floor every share to
 * the cent, then hand the leftover cents to the shares whose discarded
 * fraction was biggest. Each displayed share stays within one cent of its true
 * value, and their sum is exactly the displayed total.
 *
 * ⚠️ **It refuses to manufacture additivity that the data does not have.**
 * If the shares do not already sum to the total (beyond half a cent of
 * float slack), every share is formatted plainly and the discrepancy stays
 * visible. Hiding it would turn a formatter into an assertion about the API,
 * which is exactly the class of defect this dashboard keeps finding: the
 * displayed number is only allowed to be a rounding of the real one.
 *
 * A null/non-finite total or share disables the adjustment for the whole
 * group, for the same reason — "we don't know" cannot be balanced against.
 */
export function formatUsdShares(
  total: string | null | undefined,
  shares: readonly (string | null | undefined)[],
): string[] {
  const plain = () => shares.map(formatUsd);

  const totalValue = toFinite(total);
  if (totalValue === null) {
    return plain();
  }
  const values: number[] = [];
  for (const share of shares) {
    const value = toFinite(share);
    if (value === null) {
      return plain();
    }
    values.push(value);
  }

  // In cents, where the display lives.
  const totalCents = Math.round(totalValue * 100);
  const rawCents = values.map((v) => v * 100);
  const sumRaw = rawCents.reduce((a, b) => a + b, 0);
  if (Math.abs(sumRaw - totalValue * 100) > 0.5) {
    return plain(); // the shares are not a partition — say so by not hiding it
  }

  const floors = rawCents.map(Math.floor);
  const leftover = totalCents - floors.reduce((a, b) => a + b, 0);

  // Biggest discarded fraction first; ties keep the original order, so the
  // output is deterministic.
  const roundedUp = new Set(
    rawCents
      .map((c, i) => ({ i, frac: c - Math.floor(c) }))
      .sort((a, b) => b.frac - a.frac || a.i - b.i)
      .slice(0, Math.max(leftover, 0))
      .map(({ i }) => i),
  );

  return floors.map((c, i) =>
    FORMATTER.format((c + (roundedUp.has(i) ? 1 : 0)) / 100),
  );
}

function toFinite(value: string | null | undefined): number | null {
  if (value === null || value === undefined) {
    return null;
  }
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : null;
}