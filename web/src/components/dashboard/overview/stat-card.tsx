/**
 * Overview KPI card — label, a large value, and an optional context line.
 *
 * Visually aligned with the pool-detail `KpiCard`, but carries an optional
 * `hint` sub-line used for honest context: the TVL coverage ("348 / 359
 * priced") and the pool discovery pulse ("+52 discovered (24h)"). Cards
 * without a hint (Volume, Fees) simply omit it.
 *
 * `info` adds an ⓘ popover next to the label — the metric's definition
 * on demand. Stays a Server Component; the popover is the client island.
 */

import { getTranslations } from "next-intl/server";

import { InfoPopover } from "@/components/shared/info-popover";

const CARD_CLASS =
  "rounded-[8px] border border-sothoth-500/15 bg-cosmos-700/50 px-3 py-2 lg:px-4 lg:py-3";

const LABEL_CLASS =
  "text-[17px] font-semibold tracking-[0.2em] text-slate-400 uppercase";

const VALUE_CLASS =
  "font-display text-[21px] text-right font-bold tracking-[0.02em] text-[#f5f2ff] lg:text-[24px]";

const HINT_CLASS = "mt-1 text-right text-[13px] leading-[1.4] text-slate-500";

export async function StatCard({
  label,
  value,
  hint,
  info,
}: {
  label: string;
  value: string;
  hint?: string;
  /** Definition of the metric, shown in an ⓘ popover next to the label. */
  info?: string;
}) {
  const tShell = await getTranslations("Dashboard.shell");

  return (
    <div className={CARD_CLASS}>
      <div className="flex items-center gap-1.5">
        <p className={LABEL_CLASS}>{label}</p>
        {info && (
          <InfoPopover label={tShell("metricInfo")} iconSize={14}>
            {info}
          </InfoPopover>
        )}
      </div>
      <p className={VALUE_CLASS}>{value}</p>
      {hint ? <p className={HINT_CLASS}>{hint}</p> : null}
    </div>
  );
}
