/**
 * The pools table.
 *
 * Three columns: pair, protocol, last seen. Headers are static
 * text in this commit — sort-on-click will arrive in a separate
 * change together with search and filters.
 *
 * Wrapped in a horizontally scrollable container for narrow
 * viewports; on desktop the table fills its parent.
 */

import { getTranslations } from "next-intl/server";

import type { PoolResponse } from "@/lib/api/schema/pool";

import { PoolsTableRow } from "./pools-table-row";

const HEAD_CELL_CLASS =
  "px-4 py-3 text-left text-[11px] font-semibold tracking-[0.2em] text-slate-400 uppercase whitespace-nowrap";

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
      <table className="w-full border-collapse">
        <thead className="border-b border-sothoth-500/20 bg-cosmos-900/60">
          <tr>
            <th className={HEAD_CELL_CLASS}>{t("pair")}</th>
            <th className={HEAD_CELL_CLASS}>{t("protocol")}</th>
            <th className={HEAD_CELL_CLASS}>{t("lastSeen")}</th>
          </tr>
        </thead>
        <tbody>
          {pools.map((pool) => (
            <PoolsTableRow
              key={pool.pool_address}
              pool={pool}
              locale={locale}
            />
          ))}
        </tbody>
      </table>
    </div>
  );
}