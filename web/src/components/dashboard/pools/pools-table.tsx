import { getTranslations } from "next-intl/server";

import { KNOWN_DETECTORS } from "@/components/dashboard/signals/signal-display";
import type { PoolResponse } from "@/lib/api/schema/pool";
import type { PoolSort } from "@/lib/api/type/pagination";

import { PoolsTableRow, type SignalCellLabels } from "./pools-table-row";
import { SortableHeader } from "./sortable-header";

export const GRID_COLS =
  "grid-cols-[minmax(200px,1.8fr)_minmax(90px,0.5fr)_minmax(140px,1fr)_minmax(120px,0.9fr)_minmax(120px,0.9fr)_minmax(130px,1fr)_minmax(130px,1fr)]";
const HEAD_CELL_BASE =
  "flex items-center px-4 py-3 text-[12px] font-semibold tracking-[0.2em] text-slate-400 uppercase whitespace-nowrap";
const HEAD_CELL_CLASS = HEAD_CELL_BASE;
const HEAD_CELL_NUMERIC_CLASS = `${HEAD_CELL_BASE} justify-end`;
const HEAD_CELL_SORTABLE_CLASS = "flex items-center px-4 py-3";

type PoolsTableProps = {
  pools: PoolResponse[];
  locale: string;
  currentSort: PoolSort;
  searchParams: Record<string, string | string[] | undefined>;
};

export async function PoolsTable({
  pools,
  locale,
  currentSort,
  searchParams,
}: PoolsTableProps) {
  const t = await getTranslations("Dashboard.Pools.table");
  const tSignals = await getTranslations("Dashboard.Signals.feed");

  // Resolved once for the whole table; rows pass plain strings to the
  // client-side signal cell, keeping all i18n on the server.
  const signalLabels: SignalCellLabels = {
    tagFor: (detector) =>
      KNOWN_DETECTORS.has(detector)
        ? tSignals(`detectors.${detector}.tag`)
        : detector,
    ariaFor: (count) => t("signalsAria", { count }),
    title: t("signalsPopoverTitle"),
  };

  return (
    <div className="mx-6 overflow-x-auto rounded-[8px] border border-sothoth-500/15 bg-cosmos-900/40 lg:mx-10">
      <div role="table" className="min-w-[1030px]">
        {/* Header row */}
        <div
          role="rowgroup"
          className="border-b border-sothoth-500/20 bg-cosmos-900/60"
        >
          <div role="row" className={`grid ${GRID_COLS}`}>
            <div role="columnheader" className={HEAD_CELL_CLASS}>
              {t("pair")}
            </div>
            <div role="columnheader" className={HEAD_CELL_CLASS}>
              {t("signals")}
            </div>
            <div role="columnheader" className={HEAD_CELL_CLASS}>
              {t("protocol")}
            </div>
            <div role="columnheader" className={HEAD_CELL_NUMERIC_CLASS}>
              {t("tvl")}
            </div>
            <div role="columnheader" className={HEAD_CELL_NUMERIC_CLASS}>
              {t("volume24h")}
            </div>
            <div role="columnheader" className={HEAD_CELL_SORTABLE_CLASS}>
              <SortableHeader
                column="first_seen"
                label={t("firstSeen")}
                currentSort={currentSort}
                searchParams={searchParams}
                basePath="/pools"
              />
            </div>
            <div role="columnheader" className={HEAD_CELL_SORTABLE_CLASS}>
              <SortableHeader
                column="last_seen"
                label={t("lastSeen")}
                currentSort={currentSort}
                searchParams={searchParams}
                basePath="/pools"
              />
            </div>
          </div>
        </div>

        {/* Body rows */}
        <div role="rowgroup">
          {pools.map((pool) => (
            <PoolsTableRow
              key={pool.poolAddress}
              pool={pool}
              locale={locale}
              signalLabels={signalLabels}
            />
          ))}
        </div>
      </div>
    </div>
  );
}