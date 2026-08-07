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
 * Format a *computed* fee for display — the current fee of a pool whose base
 * fee decays over time.
 *
 * {@link formatFeeBps} exists for genesis tiers, which are chosen by a human and
 * are therefore clean (`25`, `2.5`, `5000`). A decayed fee is the output of the
 * chain's integer arithmetic and almost never is: the floor of a real linear
 * scheduler is `400.00064` bps, which that formatter renders as `"4.000006%"` —
 * correct to the digit, and indistinguishable from a rendering bug sitting under
 * a tidy `50%`.
 *
 * So this one rounds for reading rather than trimming for exactness: at most two
 * decimals of a percent (one basis point), which is finer than any fee tier in
 * use and coarse enough to look like a number someone meant.
 */
export function formatComputedFeeBps(feeBps: string | null): string {
  if (feeBps === null) return DASH;

  const bps = Number(feeBps);
  if (!Number.isFinite(bps)) return DASH;

  const percent = bps / 100;
  return `${percent.toFixed(2).replace(/\.?0+$/, "")}%`;
}

/**
 * Format the *configured* fee split (protocol / referral percents) for display,
 * e.g. `"Protocol 20% · Referral 20%"`.
 *
 * The percents are resolved as a unit from the pool account, so this returns
 * the em-dash placeholder unless both are known — "factual or absent, never
 * fake". Role labels are passed in already translated.
 *
 * A partner cut used to be shown here. It never existed: the API decoded a
 * padding byte of the pool account and always reported 0 (migration 037).
 */
export function formatFeeSplit(
  protocol: number | null,
  referral: number | null,
  labels: { protocol: string; referral: string },
): string {
  if (protocol === null || referral === null) return DASH;
  return [`${labels.protocol} ${protocol}%`, `${labels.referral} ${referral}%`].join(" · ");
}
