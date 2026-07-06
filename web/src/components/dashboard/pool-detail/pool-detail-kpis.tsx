/**
 * Pool detail page — KPI strip block.
 *
 * KPI cards, stacked on mobile:
 *
 *   - TVL              (always rendered; `—` when null)
 *   - Volume 24h       (always rendered; `—` when null)
 *   - Fees 24h         (always rendered; `—` when null) — realized
 *                      trading fee revenue over the window
 *   - Current price    (the pool's quoted A↔B spot rate, derived
 *                      server-side from `sqrt_price`; rendered only when
 *                      computable)
 *   - Pool composition (donut, rendered only when computable)
 *
 * The price and composition cards are dropped from the layout when
 * the pool has no current state yet (or, for composition, when a side
 * has no known USD price) — the grid collapses rather than showing a
 * placeholder, in line with the broader rule of "factual or absent,
 * never fake". The price is `spotPriceAInB` from the API (already
 * decimal-adjusted); the card additionally needs both symbols to label
 * the pair.
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

  // Current price: the spot price derived server-side from the pool's
  // `sqrt_price` (the true DAMM v2 concentrated-liquidity price — the
  // reserve ratio would be wrong here). Behind the `poolPriceImbalance`
  // flag. The API already returns `null` when the price isn't computable
  // (no sqrt_price, or unresolved token decimals); we additionally require
  // both symbols, which label the card in pair notation.
  const spotPrice =
    isFeatureEnabled("poolPriceImbalance") &&
      state?.spotPriceAInB != null &&
      pool.tokenA.symbol &&
      pool.tokenB.symbol
      ? Number(state.spotPriceAInB)
      : null;

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
        info={t("info.volume24h")}
      />
      <KpiCard
        label={t("fees24h")}
        valueCompact={formatUsdCompact(pool.fees24hUsd)}
        info={t("info.fees24h")}
      />
      {spotPrice !== null && (
        // Pair notation: "SOL/USDC" reads as "price of SOL in USDC",
        // i.e. token A (base) quoted in token B. `spotPriceAInB` matches.
        <KpiCard
          label={`${pool.tokenA.symbol ?? "?"}/${pool.tokenB.symbol ?? "?"}`}
          valueCompact={formatPrice(spotPrice)}
        />
      )}
    </>
  );

  // `composition` is non-null only when `state` is non-null (it is derived
  // from the reserves), so the reserves are guaranteed present here.
  const compositionCard = composition && state && (
    <PoolCompositionCard
      label={t("composition")}
      info={t("info.composition")}
      tokenA={pool.tokenA}
      tokenB={pool.tokenB}
      composition={composition}
      tvlUsd={pool.tvlUsd}
      reserveA={state.reserveA}
      reserveB={state.reserveB}
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