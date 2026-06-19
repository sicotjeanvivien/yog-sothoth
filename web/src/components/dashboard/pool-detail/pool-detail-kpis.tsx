/**
 * Pool detail page — KPI strip block.
 *
 * KPI cards, stacked on mobile:
 *
 *   - TVL              (always rendered; `—` when null)
 *   - Volume 24h       (always rendered; `—` when null)
 *   - Fees 24h         (always rendered; `—` when null) — realized
 *                      trading fee revenue over the window
 *   - Current price    (the pool's quoted A↔B rate, derived from the
 *                      latest reserves; rendered only when computable)
 *   - Pool composition (donut, rendered only when computable)
 *
 * The price and composition cards are dropped from the layout when
 * the pool has no current state yet (or, for composition, when a side
 * has no known USD price) — the grid collapses rather than showing a
 * placeholder, in line with the broader rule of "factual or absent,
 * never fake". The price needs only the reserves and the resolved
 * token metadata (decimals + symbol), not the USD prices.
 *
 * Layout: the scalar KPI cards (TVL, Volume, Fees, Price) and the
 * composition donut are two visually distinct things, so when the
 * donut is present they split into two side-by-side blocks on large
 * screens — the KPIs as a 2-column grid (two rows) on the left, the
 * donut as its own wider block on the right. Cramming all five into a
 * single five-up row squeezed the donut. Without the donut, the KPIs
 * just flow as one responsive row.
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

import { isFeatureEnabled } from "@/config/features";
import { formatUsdCompact } from "@/lib/format/format-usd";
import { computePoolComposition } from "@/lib/format/pool-composition";
import { computePoolPrice, formatPrice } from "@/lib/format/pool-price";

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
  // missing → don't render the donut card.
  const composition =
    state !== null
      ? computePoolComposition({
        reserveA: state.reserveA,
        reserveB: state.reserveB,
        decimalsA: pool.tokenA.decimals,
        decimalsB: pool.tokenB.decimals,
        priceAUsd: pool.tokenA.price?.usd ?? null,
        priceBUsd: pool.tokenB.price?.usd ?? null,
      })
      : null;

  // Current price needs only the reserves and resolved token metadata.
  // Behind the `poolPriceImbalance` flag ("Current price […] derived from
  // latest reserves"). Gate on both symbols being present: it both labels
  // the card (pair notation) and signals that the decimals are real, not
  // the 0 default the API returns for an unresolved mint (which would skew
  // the rate).
  const price =
    isFeatureEnabled("poolPriceImbalance") &&
      state !== null &&
      pool.tokenA.symbol &&
      pool.tokenB.symbol
      ? computePoolPrice({
        reserveA: state.reserveA,
        reserveB: state.reserveB,
        decimalsA: pool.tokenA.decimals,
        decimalsB: pool.tokenB.decimals,
      })
      : null;

  const kpiCount = 3 + (price ? 1 : 0);

  const kpiCards = (
    <>
      <KpiCard label={t("tvl")} valueCompact={formatUsdCompact(pool.tvlUsd)} />
      <KpiCard
        label={t("volume24h")}
        valueCompact={formatUsdCompact(pool.volume24hUsd)}
      />
      <KpiCard
        label={t("fees24h")}
        valueCompact={formatUsdCompact(pool.fees24hUsd)}
      />
      {price && (
        // Pair notation: "SOL/USDC" reads as "price of SOL in USDC",
        // i.e. token A (base) quoted in token B. `priceAInB` matches.
        <KpiCard
          label={`${pool.tokenA.symbol ?? "?"}/${pool.tokenB.symbol ?? "?"}`}
          valueCompact={formatPrice(price.priceAInB)}
        />
      )}
    </>
  );

  const compositionCard = composition && (
    <PoolCompositionCard
      label={t("composition")}
      tokenA={pool.tokenA}
      tokenB={pool.tokenB}
      composition={composition}
      tvlUsd={pool.tvlUsd}
      className="lg:col-span-4 lg:h-full"
    />
  );

  return (
    <section className="px-6 lg:px-10">
      {compositionCard ? (
        // Two blocks side by side on large screens: the KPI cards as a
        // 2-up grid (two rows) over five ninths, the donut its own four
        // ninths. Both columns stretch to the same height (default grid
        // `items-stretch`); the KPI grid fills it via `grid-rows-2` and
        // the donut card via `h-full`, so the two blocks line up exactly.
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-9">
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:col-span-5 lg:h-full lg:grid-rows-2">
            {kpiCards}
          </div>
          {compositionCard}
        </div>
      ) : (
        // No donut → KPIs flow as a single responsive row.
        <div
          className={`grid grid-cols-1 gap-4 sm:grid-cols-2 ${kpiCount === 4 ? "lg:grid-cols-4" : "lg:grid-cols-3"}`}
        >
          {kpiCards}
        </div>
      )}
    </section>
  );
}