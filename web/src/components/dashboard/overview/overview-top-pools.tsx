/**
 * Overview page — top pools by 24h volume.
 *
 * Self-contained async Server Component: fetches `GET /api/pools/top` itself
 * and degrades to a `BlockError` on failure so a ranking hiccup never takes
 * down the KPI strip above it. The page just mounts `<OverviewTopPools />`.
 *
 * A compact ranked table — rank · pair · Volume 24h · TVL — each row linking
 * to the pool detail. This is the *only* volume-ranked view of pools: the
 * `/pools` list can only sort by first/last-seen (keyset on timestamps), so
 * there is no "see all by volume" to link to.
 *
 * USD cells render `—` when null (the format helper owns the null check).
 */

import { getTranslations } from "next-intl/server";

import { BlockError } from "@/components/dashboard/block-error";
import { PoolPairCell } from "@/components/dashboard/pools/pool-pair-cell";
import { Link } from "@/i18n/navigation";
import { ApiClientError } from "@/lib/api/errors";
import type { PoolResponse } from "@/lib/api/schema/pool";
import { fetchTopPools } from "@/lib/api/server/top-pools";
import { formatUsdCompact } from "@/lib/format/format-usd";

const GRID_COLS = "grid-cols-[2rem_1fr_auto_auto]";

const CELL = "px-4 py-3 text-[14px] flex items-center";
const CELL_NUM = `${CELL} justify-end font-mono text-slate-300`;
const HEAD = "px-4 py-3 text-[11px] font-semibold tracking-[0.2em] text-slate-500 uppercase flex items-center";
const HEAD_NUM = `${HEAD} justify-end`;

export async function OverviewTopPools() {
  const t = await getTranslations("Dashboard.Overview.topPools");

  let pools: PoolResponse[];
  try {
    pools = await fetchTopPools();
  } catch (err) {
    if (err instanceof ApiClientError) {
      return (
        <section className="mt-8 px-6 lg:px-10">
          <BlockError title={t("title")} kind={err.details.kind} />
        </section>
      );
    }
    throw err;
  }

  return (
    <section className="mt-8 px-6 lg:px-10">
      <h2 className="mb-4 text-[12px] font-semibold tracking-[0.28em] text-slate-400 uppercase">
        {t("title")}
      </h2>

      {pools.length === 0 ? (
        <p className="rounded-[8px] border border-sothoth-500/15 bg-cosmos-700/40 px-4 py-6 text-[14px] text-slate-400">
          {t("empty")}
        </p>
      ) : (
        <div
          role="table"
          className="overflow-hidden rounded-[8px] border border-sothoth-500/15 bg-cosmos-700/40"
        >
          <div role="row" className={`grid ${GRID_COLS} border-b border-sothoth-500/15`}>
            <div role="columnheader" className={HEAD}>
              #
            </div>
            <div role="columnheader" className={HEAD}>
              {t("pair")}
            </div>
            <div role="columnheader" className={HEAD_NUM}>
              {t("volume24h")}
            </div>
            <div role="columnheader" className={HEAD_NUM}>
              {t("tvl")}
            </div>
          </div>

          {pools.map((pool, index) => (
            <Link
              key={pool.poolAddress}
              role="row"
              href={`/pools/${pool.poolAddress}`}
              className={`grid ${GRID_COLS} border-b border-sothoth-500/10 transition-colors last:border-b-0 hover:bg-sothoth-500/[0.04]`}
            >
              <div role="cell" className={`${CELL} justify-center font-mono text-slate-500`}>
                {index + 1}
              </div>
              <div role="cell" className={CELL}>
                <PoolPairCell tokenA={pool.tokenA} tokenB={pool.tokenB} />
              </div>
              <div role="cell" className={CELL_NUM}>
                {formatUsdCompact(pool.volume24hUsd)}
              </div>
              <div role="cell" className={CELL_NUM}>
                {formatUsdCompact(pool.tvlUsd)}
              </div>
            </Link>
          ))}
        </div>
      )}
    </section>
  );
}
