/**
 * The pool detail's KPI strip: TVL, volume, fees, and when computable the spot price and the
 * composition donut — dropped, not faked, otherwise. `state` is null for a pool with no
 * current state (the latest-state 404).
 */

import { getTranslations } from "next-intl/server";

import type { PoolCurrentStateResponse, PoolResponse } from "@/lib/api/schema/zod.gen";

import { isFeatureEnabled } from "@/config/features";
import { formatUsdCompact } from "@/lib/format/format-usd";
import { volumeCoverage } from "@/lib/coverage/volume-coverage";
import { computePoolComposition } from "@/lib/format/pool-composition";
import { formatPrice } from "@/lib/format/pool-price";

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

  // Composition needs both sides valued. Anything missing → no donut card.
  const composition =
    state !== null
      ? computePoolComposition({
        valueAUsd: state.valueAUsd,
        valueBUsd: state.valueBUsd,
      })
      : null;

  // From `sqrt_price`, not the reserve ratio, which would be wrong for a concentrated pool.
  // Both symbols are required too: they label the card.
  const spotPrice =
    isFeatureEnabled("poolPriceImbalance") &&
      state?.spotPriceAInB != null &&
      pool.tokenA.symbol &&
      pool.tokenB.symbol
      ? Number(state.spotPriceAInB)
      : null;

  // A KPI states the coverage even when complete, where a table row marks only a partial one.
  const coverage = volumeCoverage(pool);
  const coverageHint =
    coverage !== null
      ? t("bucketCoverage", {
        priced: coverage.priced,
        total: coverage.total,
      })
      : undefined;

  const kpiCount = 3 + (spotPrice !== null ? 1 : 0);

  const kpiCards = (
    <>
      <KpiCard
        label={t("tvl")}
        valueCompact={formatUsdCompact(pool.tvlUsd)}
        info={t("info.tvl")}
      />
      <KpiCard
        label={t("volume24h")}
        valueCompact={formatUsdCompact(pool.volume24hUsd)}
        hint={coverageHint}
        info={t("info.volume24h")}
      />
      {/* The volume's coverage hint: fees share its valuation, so its gaps too. */}
      <KpiCard
        label={t("fees24h")}
        valueCompact={formatUsdCompact(pool.fees24hUsd)}
        hint={coverageHint}
        info={t("info.fees24h")}
      />
      {spotPrice !== null && (
        // "SOL/USDC" is the price of A in B, as `spotPriceAInB` is.
        <KpiCard
          label={`${pool.tokenA.symbol ?? "?"}/${pool.tokenB.symbol ?? "?"}`}
          valueCompact={formatPrice(spotPrice)}
        />
      )}
    </>
  );

  const compositionCard = composition && state && (
    <PoolCompositionCard
      label={t("composition")}
      info={t("info.composition")}
      tokenA={pool.tokenA}
      tokenB={pool.tokenB}
      composition={composition}
      reserveA={state.reserveA}
      reserveB={state.reserveB}
      className="lg:col-span-4 lg:h-full"
    />
  );

  return (
    <section className="px-6 lg:px-10">
      {compositionCard ? (
        // Both columns stretch alike: `grid-rows-2` fills the KPI grid, `h-full` the donut.
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-9">
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:col-span-5 lg:h-full lg:grid-rows-2">
            {kpiCards}
          </div>
          {compositionCard}
        </div>
      ) : (
        <div
          className={`grid grid-cols-1 gap-4 sm:grid-cols-2 ${kpiCount === 4 ? "lg:grid-cols-4" : "lg:grid-cols-3"}`}
        >
          {kpiCards}
        </div>
      )}
    </section>
  );
}