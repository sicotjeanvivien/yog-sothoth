/**
 * Format a pool's base trading fee for display.
 *
 * The API exposes the fee as `feeBps` — basis points, as a precision-safe
 * decimal string (e.g. `"25"`, `"2.5"`, `"5000"`). Traders read fee tiers as a
 * percentage, so we show the percent form: `25` bps → `"0.25%"`, `5000` bps →
 * `"50%"`. Trailing zeros are trimmed so clean tiers stay clean.
 *
 * Returns the em-dash placeholder when the fee is unknown (null — the pool's
 * `InitializePool` event has not been indexed yet), in line with the
 * "factual or absent, never fake" rule.
 */
const DASH = "—";

export function formatFeeBps(feeBps: string | null): string {
  if (feeBps === null) return DASH;

  const bps = Number(feeBps);
  if (!Number.isFinite(bps)) return DASH;

  // bps → percent. Fee values are small and clean, so Number precision is
  // ample for display; fixed(6) then trim avoids float noise like 0.1 + 0.2.
  const percent = bps / 100;
  const trimmed = percent.toFixed(6).replace(/\.?0+$/, "");
  return `${trimmed}%`;
}

/**
 * Format the *configured* fee split (protocol / partner / referral percents)
 * for display, e.g. `"Protocol 20% · Partner 0% · Referral 20%"`.
 *
 * The percents are resolved as a unit from the pool account, so this returns
 * the em-dash placeholder unless all three are known — "factual or absent,
 * never fake". Role labels are passed in already translated.
 */
export function formatFeeSplit(
  protocol: number | null,
  partner: number | null,
  referral: number | null,
  labels: { protocol: string; partner: string; referral: string },
): string {
  if (protocol === null || partner === null || referral === null) return DASH;
  return [
    `${labels.protocol} ${protocol}%`,
    `${labels.partner} ${partner}%`,
    `${labels.referral} ${referral}%`,
  ].join(" · ");
}
