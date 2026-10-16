/**
 * A base fee tier, from bps to a trimmed percent: `"25"` → `"0.25%"`. "—" while the pool is
 * unresolved.
 */
const DASH = "—";

export function formatFeeBps(feeBps: string | null): string {
  if (feeBps === null) return DASH;

  const bps = Number(feeBps);
  if (!Number.isFinite(bps)) return DASH;

  // `toFixed(6)` then trim: no float noise like 0.1 + 0.2.
  const percent = bps / 100;
  const trimmed = percent.toFixed(6).replace(/\.?0+$/, "");
  return `${trimmed}%`;
}

/**
 * A computed, decaying fee, rounded to one basis point: chain arithmetic gives `400.00064`,
 * which {@link formatFeeBps} would render as `"4.000006%"`.
 */
export function formatComputedFeeBps(feeBps: string | null): string {
  if (feeBps === null) return DASH;

  const bps = Number(feeBps);
  if (!Number.isFinite(bps)) return DASH;

  const percent = bps / 100;
  return `${percent.toFixed(2).replace(/\.?0+$/, "")}%`;
}

/**
 * The configured split, `"Protocol 20% · Referral 20%"`, or "—" unless both are known; the
 * role labels come translated.
 */
export function formatFeeSplit(
  protocol: number | null,
  referral: number | null,
  labels: { protocol: string; referral: string },
): string {
  if (protocol === null || referral === null) return DASH;
  return [`${labels.protocol} ${protocol}%`, `${labels.referral} ${referral}%`].join(" · ");
}
