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



  return (
    <div className="mx-auto max-w-7xl px-6 py-10 lg:px-10 lg:py-12">

    </div>
  );
}
