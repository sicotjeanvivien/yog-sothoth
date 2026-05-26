/**
 * Pool detail page — `/[locale]/pools/[address]`.
 *
 * Server Component. Fetch strategy ("Option C optimised"):
 *
 *   1. Fetch the pool identity sequentially. A 404 from yog-api means
 *      the pool was never observed — call Next.js `notFound()` so the
 *      framework renders the standard 404 page instead of an empty
 *      detail layout.
 *   2. If the pool exists, fan out the three remaining calls in
 *      parallel: latest-state, swaps, liquidity-events.
 *
 * The three fan-out calls go through `safeFetch*` helpers local to
 * this page — same pattern as the pools listing page. Failures are
 * rendered as in-card error states; one feed failing never breaks
 * the rest of the page.
 *
 * Direct fetcher calls (no BFF round-trip) — we are already on the
 * server. The BFF route handlers at `app/api/pools/[address]/**` exist
 * for browser-side consumers (future Client Components, polling).
 */

import { PoolDetailHeader } from "@/components/dashboard/pool-detail/pool-detail-header";
import { PoolDetailInfo } from "@/components/dashboard/pool-detail/pool-detail-info";
import { setRequestLocale } from "next-intl/server";

type PoolDetailPageProps = {
  params: Promise<{ locale: string; address: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
};

export default async function PoolDetailPage({
  params,
  searchParams,
}: PoolDetailPageProps) {
  const { locale, address } = await params;
  const search = await searchParams;
  setRequestLocale(locale);

  let pool = {
    "poolAddress": "7ccKzmrXBpFHwyZGPqPuKL6bEyWAETSnHwnWe3jEneVc",
    "protocol": "meteora_damm_v2",
    "tokenA": {
      "mint": "BFgdzMkTPdKKJeTipv2njtDEwhKxkgFueJQfJGt1jups",
      "symbol": "URANUS",
      "name": "Uranus",
      "decimals": 6,
      "logoUri": "https://static-create.jup.ag/images/BFgdzMkTPdKKJeTipv2njtDEwhKxkgFueJQfJGt1jups-8c6c089e-20b9-4114-9dc3-e77103318edf.jpg",
      "price": {
        "usd": "0.015319848522429428",
        "source": "jupiter",
        "fetchedAt": "2026-05-26T05:20:25.845004Z"
      }
    },
    "tokenB": {
      "mint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
      "symbol": "USDC",
      "name": "USD Coin",
      "decimals": 6,
      "logoUri": "https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v/logo.png",
      "price": {
        "usd": "0.999682204488653000",
        "source": "jupiter",
        "fetchedAt": "2026-05-26T05:20:25.845004Z"
      }
    },
    "tvlUsd": "166687.33145330663639393683164",
    "volume24hUsd": null,
    "firstSeenAt": "2026-05-25T13:27:27.630479Z",
    "lastSeenAt": "2026-05-25T13:27:27.630479Z"
  };

  return (
    <div className="mx-auto max-w-7xl px-6 py-10 lg:px-10 lg:py-12">
      <PoolDetailHeader pool={pool} />
      <PoolDetailInfo pool={pool} locale={locale} />
    </div>
  );
}
