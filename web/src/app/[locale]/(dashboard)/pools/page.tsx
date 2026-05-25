/**
 * Pools page (`/[locale]/(dashboard)/pools`).
 *
 * Server Component. Calls `fetchPools` directly from `lib/api`; no
 * BFF round-trip is needed since the call already happens on the
 * Next.js server. The BFF route handlers are for browser-initiated
 * requests, which this page doesn't perform.
 *
 * Three display states are mutually exclusive:
 *
 *   - error  → `PoolsError` (driven by `ApiClientError.details.kind`)
 *   - empty  → `PoolsEmpty`
 *   - filled → `PoolsTable`
 *
 * Search, filters, sortable column headers, and pagination beyond
 * the first page are intentionally out of scope here — each lands
 * in a dedicated commit.
 */

import { setRequestLocale, getTranslations } from "next-intl/server";
import type { Metadata } from "next";

import { PoolsHeader } from "@/components/dashboard/pools/pools-header";
import { PoolsTable } from "@/components/dashboard/pools/pools-table";
import { PoolsEmpty } from "@/components/dashboard/pools/pools-empty";
import { PoolsError } from "@/components/dashboard/pools/pools-error";

import { fetchPools } from "@/lib/api/pools";
import { ApiClientError, type ApiClientErrorKind } from "@/lib/api/errors";
import type { PoolsPageResponse } from "@/lib/api/schema/page";

// ── Page metadata ─────────────────────────────────────────────────────

type PoolsPageProps = {
  params: Promise<{ locale: string }>;
};

export async function generateMetadata({
  params,
}: PoolsPageProps): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({
    locale,
    namespace: "Dashboard.Pools.page",
  });
  return {
    title: `${t("title")} — Yog-Scope`,
    description: t("description"),
  };
}

// ── Fetch result type ────────────────────────────────────────────────
//
// The page receives either the API page or a typed error kind.
// Anything else (an unexpected non-`ApiClientError` throw) bubbles
// up to the Next.js error boundary, which is the right behaviour:
// unknown failures should not be silently swallowed into the
// "unable to load" UI state.

type FetchOutcome =
  | { kind: "ok"; data: PoolsPageResponse }
  | { kind: "error"; reason: ApiClientErrorKind };

async function load(): Promise<FetchOutcome> {
  try {
    const data = await fetchPools({});
    return { kind: "ok", data };
  } catch (err) {
    if (err instanceof ApiClientError) {
      return { kind: "error", reason: err.details.kind };
    }
    // Re-throw anything that isn't an ApiClientError — the page
    // should not paper over unknown errors.
    throw err;
  }
}

// ── Page ──────────────────────────────────────────────────────────────

export default async function PoolsPage({ params }: PoolsPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);

  const outcome = await load();

  return (
    <div className="pb-16">
      <PoolsHeader />

      {outcome.kind === "error" ? (
        <PoolsError kind={outcome.reason} />
      ) : outcome.data.items.length === 0 ? (
        <PoolsEmpty />
      ) : (
        <PoolsTable pools={outcome.data.items} locale={locale} />
      )}
    </div>
  );
}