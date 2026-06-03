/**
 * Generic KPI card — small, two-line block: an uppercase label and
 * a large value.
 *
 * Used at the top of the pool detail page for TVL and 24h volume,
 * but designed to be reusable for any future scalar metric on the
 * dashboard.
 *
 * Stays a Server Component — no interactivity, just typography
 * inside a styled container.
 */

const CARD_CLASS =
  "rounded-[8px] border border-sothoth-500/15 bg-cosmos-900/40 px-5 py-4 lg:px-6 lg:py-5";

const LABEL_CLASS =
  "text-[21px] font-semibold tracking-[0.2em] text-slate-400 uppercase";

const VALUE_COMPACT_CLASS =
  "mt-2 font-display text-[28px] text-right font-bold tracking-[0.02em] text-[#f5f2ff] lg:text-[32px]";

const VALUE_CLASS =
  "mt-2 font-display text-[14px] text-right font-bold tracking-[0.02em] text-slate-400 lg:text-[17px]";

export function KpiCard({
  label,
  valueCompact,
  value,
}: {
  label: string;
  valueCompact: string;
  value: string;
}) {
  return (
    <div className={CARD_CLASS}>
      <p className={LABEL_CLASS}>{label}</p>
      <p className={VALUE_COMPACT_CLASS}>{valueCompact}</p>
      <p className={VALUE_CLASS}>{value}</p>
    </div>
  );
}