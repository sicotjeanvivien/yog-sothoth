/**
 * Overview page — top pools, ranked by volume or TVL.
 *
 * Self-contained async Server Component: fetches `GET /api/pools/top` itself
 * and degrades to a `BlockError` on failure so a ranking hiccup never takes
 * down the KPI strip above it. Layout (spacing, the side-by-side grid with
 * the latest-signals block) belongs to the page — this renders content only.
 *
 * A compact ranked table — rank · pair · Volume 24h · TVL — each row linking
 * to the pool detail. The two numeric headers are clickable: they re-rank the
 * strip by flow (`volume_24h`, default) or depth (`tvl`) via a `?rank=` URL
 * param resolved by the page. This is the *only* metric-ranked view of pools:
 * the `/pools` list can only sort by first/last-seen (keyset on timestamps).
 *
 * USD cells render `—` when null (the format helper owns the null check). A
 * volume that covers only part of the hours it traded carries a coverage mark;
 * the block's ⓘ says the rest — that a pool with no valuable hour at all is
 * missing from the ranking entirely, which is the half a per-row mark cannot
 * express.
 */

import { getTranslations } from "next-intl/server";

import { BlockError } from "@/components/dashboard/block-error";
import { PoolPairCell } from "@/components/dashboard/pools/pool-pair-cell";
import { VolumeCoverageMark } from "@/components/dashboard/pools/volume-coverage-mark";
import { InfoPopover } from "@/components/shared/info-popover";
import { Link } from "@/i18n/navigation";
import { volumeCoverage } from "@/lib/coverage/volume-coverage";
import { ApiClientError } from "@/lib/api/errors";
import type { PoolResponse } from "@/lib/api/schema/pool";
import { fetchTopPools, type PoolRankMetric } from "@/lib/api/server/top-pools";
import { formatUsdCompact } from "@/lib/format/format-usd";

import { OverviewRankHeader } from "./overview-rank-header";

const GRID_COLS = "grid-cols-[2rem_1fr_auto_auto]";

const CELL = "px-4 py-3 text-[14px] flex items-center";
const CELL_NUM = `${CELL} justify-end font-mono text-slate-300`;
const HEAD = "px-4 py-3 text-[12px] font-semibold tracking-[0.2em] text-slate-500 uppercase flex items-center";
const HEAD_NUM = `${HEAD} justify-end`;

type OverviewTopPoolsProps = {
  /** Active ranking metric, resolved from the URL by the page. */
  metric: PoolRankMetric;
  /** Current search params — forwarded to the clickable rank headers. */
  searchParams: Record<string, string | string[] | undefined>;
};

export async function OverviewTopPools({
  metric,
  searchParams,
}: OverviewTopPoolsProps) {
  const t = await getTranslations("Dashboard.Overview.topPools");

  let pools: PoolResponse[];
  try {
    pools = await fetchTopPools(metric);
  } catch (err) {
    if (err instanceof ApiClientError) {
      return <BlockError title={t("title")} kind={err.details.kind} />;
    }
    throw err;
  }

  return (
    <div>
      {/* The ⓘ carries what no row can: this ranking OMITS the pools whose
          every traded hour was unvaluable — they are absent, not last. */}
      <div className="mb-4 flex items-center gap-2">
        <h2 className="text-[13px] font-semibold tracking-[0.28em] text-slate-400 uppercase">
          {t("title")}
        </h2>
        <InfoPopover label={t("title")}>{t("info")}</InfoPopover>
      </div>

      {pools.length === 0 ? (
        <p className="rounded-[8px] border border-sothoth-500/15 bg-cosmos-700/40 px-4 py-6 text-[14px] text-slate-400">
          {metric === "tvl" ? t("emptyTvl") : t("emptyVolume")}
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
              <OverviewRankHeader
                metric="volume_24h"
                label={t("volume24h")}
                activeMetric={metric}
                searchParams={searchParams}
              />
            </div>
            <div role="columnheader" className={HEAD_NUM}>
              <OverviewRankHeader
                metric="tvl"
                label={t("tvl")}
                activeMetric={metric}
                searchParams={searchParams}
              />
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
                <VolumeCoverageMark
                  coverage={volumeCoverage(pool)}
                  labelFor={(priced, total) =>
                    t("volumeCoverage", { priced, total })
                  }
                />
              </div>
              <div role="cell" className={CELL_NUM}>
                {formatUsdCompact(pool.tvlUsd)}
              </div>
            </Link>
          ))}
        </div>
      )}
    </div>
  );
}
