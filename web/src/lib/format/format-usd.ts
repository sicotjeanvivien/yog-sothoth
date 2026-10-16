/**
 * USD amounts from the API's decimal strings: `formatUsdCompact("845200000.50")` is
 * `"$845.2M"`, a null or NaN is `"—"`. ⚠️ Locale forced to en-US, so the separator stays a dot.
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

/** `FORMATTER`'s rounding without symbol or separators, so it parses back — see `displayedCents`. */
const CENTS_FORMATTER = new Intl.NumberFormat("en-US", {
  notation: "standard",
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
  useGrouping: false,
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
 * Formats shares of a total so the displayed shares add up to the displayed total (largest
 * remainder). ⚠️ Shares that do not already sum to the total, or any null or non-finite value, are
 * formatted plainly: the discrepancy stays visible.
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

  // ⚠️ The cents the total row shows, not `total * 100`: `0.145 * 100` rounds down in binary
  // while `Intl` rounds half-expand.
  const totalCents = displayedCents(totalValue);
  const rawCents = values.map((v) => v * 100);
  const sumRaw = rawCents.reduce((a, b) => a + b, 0);
  const exactCents = totalValue * 100;

  // ⚠️ A relative, tiny tolerance: it absorbs only `parseFloat`'s ulps, not real API drift.
  const slack = Math.max(1e-6, Math.abs(exactCents) * 1e-12);
  if (Math.abs(sumRaw - exactCents) > slack) {
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

/** The cents this amount is rendered as, through the formatter itself so the two cannot drift. */
function displayedCents(value: number): number {
  return Math.round(Number.parseFloat(CENTS_FORMATTER.format(value)) * 100);
}

function toFinite(value: string | null | undefined): number | null {
  if (value === null || value === undefined) {
    return null;
  }
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : null;
}