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
 * dashboard. `info` adds an ⓘ popover next to the label — the
 * metric's definition on demand.
 *
 * Stays a Server Component; the popover is the client island.
 */

import { getTranslations } from "next-intl/server";

import { InfoPopover } from "@/components/shared/info-popover";

const CARD_CLASS =
  "flex h-full flex-col justify-center rounded-[8px] border border-sothoth-500/15 bg-cosmos-700/50 px-3 py-2 lg:px-4 lg:py-3";

const LABEL_CLASS =
  "text-[17px] font-semibold tracking-[0.2em] text-slate-400 uppercase";

const VALUE_COMPACT_CLASS =
  "font-display text-[21px] text-right font-bold tracking-[0.02em] text-[#f5f2ff] lg:text-[24px]";

export async function KpiCard({
  label,
  valueCompact,
  info,
}: {
  label: string;
  valueCompact: string;
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
    </div>
  );
}
