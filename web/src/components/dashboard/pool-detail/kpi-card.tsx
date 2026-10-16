/**
 * A KPI card: a label and a compact value, the exact figure being in "Pool analytics". A
 * Server Component; the ⓘ popover is the client island.
 */

import { getTranslations } from "next-intl/server";

import { InfoPopover } from "@/components/shared/info-popover";

const CARD_CLASS =
  "flex h-full flex-col justify-center rounded-[8px] border border-sothoth-500/15 bg-cosmos-700/50 px-3 py-2 lg:px-4 lg:py-3";

const LABEL_CLASS =
  "text-[17px] font-semibold tracking-[0.2em] text-slate-400 uppercase";

const VALUE_COMPACT_CLASS =
  "font-display text-[21px] text-right font-bold tracking-[0.02em] text-[#f5f2ff] lg:text-[24px]";

const HINT_CLASS = "mt-1 text-right text-[13px] leading-[1.4] text-slate-500";

export async function KpiCard({
  label,
  valueCompact,
  hint,
  info,
}: {
  label: string;
  valueCompact: string;
  /** A context line under the value, such as the coverage ("14 / 24 hours priced"). */
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
      <p className={VALUE_COMPACT_CLASS}>{valueCompact}</p>
      {hint ? <p className={HINT_CLASS}>{hint}</p> : null}
    </div>
  );
}
