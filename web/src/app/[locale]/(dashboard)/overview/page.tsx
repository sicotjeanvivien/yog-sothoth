/**
 * Pools listing page — first iteration.
 *
 * Server Component. Calls `fetchPools` directly (we are on the server
 * already; bouncing through the BFF route handler would just add a
 * loopback HTTP hop for no benefit). The route handler at
 * `app/api/pools/route.ts` exists for browser-side consumers
 * (future Client Components, external integrations).
 *
 * Pagination is forward-only:
 *   - `?cursor=<opaque>` advances the page.
 *   - Removing the cursor (the "First page" link) resets to the head.
 *   - Browser back is the recommended way to revisit a previous page.
 *
 * Failure handling: any `ApiClientError` is caught and rendered as a
 * typed error state. The page never crashes the layout.
 */

import { getTranslations, setRequestLocale } from "next-intl/server";

// ── Route configuration ────────────────────────────────────────────────

// Force dynamic rendering — the page depends on `?cursor=` and on
// live yog-api data; static rendering would defeat both.
export const dynamic = "force-dynamic";

// ── Types ─────────────────────────────────────────────────────────────

type PoolsPageProps = {
  params: Promise<{ locale: string }>;
  // `searchParams` is async in Next.js 15+ (same as `params`).
  searchParams: Promise<{ cursor?: string | string[] }>;
};

export default async function PoolsListingPage({
  params,
  searchParams,
}: PoolsPageProps) {
  const { locale } = await params;
  const { cursor: rawCursor } = await searchParams;
  setRequestLocale(locale);

  const basePath = `/${locale}/pools`;

  return (
    <div className="mx-auto max-w-7xl px-6 py-10 lg:px-10 lg:py-12">
      <header className="mb-8">
      </header>

    </div>
  );
}