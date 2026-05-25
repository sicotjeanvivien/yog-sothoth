/**
 * The pools table.
 *
 * Rendered as a CSS grid rather than a native `<table>`. The
 * native option forced us to wrap each row in `<Link>` styled as
 * `display: table-row`, which produced an `<a>` directly inside a
 * `<tbody>` — invalid HTML and a guaranteed hydration mismatch.
 *
 * The grid layout keeps the table semantics through ARIA roles
 * (`role="table"` / `"rowgroup"` / `"row"` / `"columnheader"` /
 * `"cell"`) so screen readers still announce the structure, while
 * allowing each row to be a plain `<a>` element wrapping its
 * cells.
 *
 * Five columns: pair, protocol, TVL, 24h volume, last seen.
 * Headers are static in this commit — sort-on-click will arrive
 * with search and filters.
 *
 * The column template is exported so the row component can share
 * it: any change to widths happens in one place.
 */

import { getTranslations } from "next-intl/server";

import type { PoolResponse } from "@/lib/api/schema/pool";

import { PoolsTableRow } from "./pools-table-row";

/**
 * Column widths. Pair gets the most room because it carries two
 * logos and two symbols; the numeric columns are wider than the
 * timestamp to fit "$1.28M" comfortably. The `minmax(..., Nfr)`
 * form keeps the columns from collapsing on narrow viewports while
 * still letting them grow.
 */
export const GRID_COLS =
  "grid-cols-[minmax(220px,2fr)_minmax(160px,1fr)_minmax(130px,1fr)_minmax(130px,1fr)_minmax(140px,1fr)]";

const HEAD_CELL_CLASS =
  "px-4 py-3 text-left text-[11px] font-semibold tracking-[0.2em] text-slate-400 uppercase whitespace-nowrap";

const HEAD_CELL_NUMERIC_CLASS = `${HEAD_CELL_CLASS} text-right`;

export async function PoolsTable({
  pools,
  locale,
}: {
  pools: PoolResponse[];
  locale: string;
}) {
  const t = await getTranslations("Dashboard.Pools.table");

  return (
    <div className="mx-6 overflow-x-auto rounded-[8px] border border-sothoth-500/15 bg-cosmos-900/40 lg:mx-10">
      <div role="table" className="min-w-[860px]">
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
              {t("protocol")}
            </div>
            <div role="columnheader" className={HEAD_CELL_NUMERIC_CLASS}>
              {t("tvl")}
            </div>
            <div role="columnheader" className={HEAD_CELL_NUMERIC_CLASS}>
              {t("volume24h")}
            </div>
            <div role="columnheader" className={HEAD_CELL_CLASS}>
              {t("lastSeen")}
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
            />
          ))}
        </div>
      </div>
    </div>
  );
}