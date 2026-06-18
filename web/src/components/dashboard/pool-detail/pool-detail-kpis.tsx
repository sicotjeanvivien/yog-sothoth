/**
 * Pool detail page — KPI strip block.
 *
 * KPI cards, stacked on mobile:
 *
 *   - TVL              (always rendered; `—` when null)
 *   - Volume 24h       (always rendered; `—` when null)
 *   - Fees 24h         (always rendered; `—` when null) — realized
 *                      trading fee revenue over the window
 *   - Pool composition (donut, rendered only when computable)
 *
 * The composition card is dropped from the layout when either side
 * of the pool has no known price or when the pool has no current
 * state yet — the grid collapses to two cards rather than showing
 * a placeholder, in line with the broader rule of "factual or
 * absent, never fake".
 *
 * Inputs:
 *   - `pool`: identity + analytics from `GET /api/pools/{address}`
 *   - `state`: current reserves from `GET /api/pools/{address}/latest-state`,
 *              null when the endpoint returned 404 (pool observed
 *              but no swap/liquidity event yet).
 */

import { getTranslations } from "next-intl/server";

import type { PoolResponse } from "@/lib/api/schema/pool";
import type { PoolCurrentStateResponse } from "@/lib/api/schema/pool-current-state";

import { formatUsdCompact } from "@/lib/format/format-usd";
import { computePoolComposition } from "@/lib/format/pool-composition";

import { KpiCard } from "./kpi-card";
import { PoolCompositionCard } from "./pool-composition-card";

export async function PoolDetailKpis({
  pool,
  state,
}: {
  pool: PoolResponse;
  state: PoolCurrentStateResponse | null;
}) {
  const t = await getTranslations("Dashboard.PoolDetail.kpis");

  // Composition needs the current state AND both prices. Anything
  // missing → don't render the third card.
  const composition =
    state !== null
      ? computePoolComposition({
        reserveA: String(state.reserveA),
        reserveB: String(state.reserveB),
        decimalsA: pool.tokenA.decimals,
        decimalsB: pool.tokenB.decimals,
        priceAUsd: pool.tokenA.price?.usd ?? null,
        priceBUsd: pool.tokenB.price?.usd ?? null,
      })
      : null;

  // Grid layout: 1 column on mobile. Three base KPIs (TVL, Volume,
  // Fees) always render — 3 cols; with the composition donut it
  // becomes a 4th card — 4 cols. Tailwind class chosen accordingly.
  const gridClass = composition
    ? "grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4"
    : "grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3";

  return (
    <section className="px-6 lg:px-10">
      <div className={gridClass}>
        <KpiCard
          label={t("tvl")}
          valueCompact={formatUsdCompact(pool.tvlUsd)}
        />
        <KpiCard
          label={t("volume24h")}
          valueCompact={formatUsdCompact(pool.volume24hUsd)}
        />
        <KpiCard
          label={t("fees24h")}
          valueCompact={formatUsdCompact(pool.fees24hUsd)}
        />
        {composition && (
          <PoolCompositionCard
            label={t("composition")}
            tokenA={pool.tokenA}
            tokenB={pool.tokenB}
            composition={composition}
            tvlUsd={pool.tvlUsd}
          />
        )}
      </div>
    </section>
  );
}