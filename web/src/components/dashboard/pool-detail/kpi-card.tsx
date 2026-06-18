/**
 * Generic KPI card — small, two-line block: an uppercase label and
 * a large value.
 *
 * Shows the metric at a glance, as an *order of magnitude* (compact,
 * e.g. "$1.2M"). The exact figure lives in the "Pool analytics" card
 * below — the KPI strip is for scanning, the analytics card for
 * precise reading.
 *
 * Used at the top of the pool detail page for TVL and 24h volume,
 * but designed to be reusable for any future scalar metric on the
 * dashboard.
 *
 * Stays a Server Component — no interactivity, just typography
 * inside a styled container.
 */

const CARD_CLASS =
  "rounded-[8px] border border-sothoth-500/15 bg-cosmos-700/50 px-3 py-2 lg:px-4 lg:py-3";

const LABEL_CLASS =
  "text-[17px] font-semibold tracking-[0.2em] text-slate-400 uppercase";

const VALUE_COMPACT_CLASS =
  "font-display text-[21px] text-right font-bold tracking-[0.02em] text-[#f5f2ff] lg:text-[24px]";

export function KpiCard({
  label,
  valueCompact,
}: {
  label: string;
  valueCompact: string;
}) {
  return (
    <div className={CARD_CLASS}>
      <p className={LABEL_CLASS}>{label}</p>
      <p className={VALUE_COMPACT_CLASS}>{valueCompact}</p>
    </div>
  );
}