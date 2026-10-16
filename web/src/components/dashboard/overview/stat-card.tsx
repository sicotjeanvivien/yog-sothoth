/**
 * The Overview's KPI card: a label, a large value, and an optional `hint` line for coverage
 * or discovery. A Server Component; the ⓘ popover is the client island.
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
  hint?: string | undefined;
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
