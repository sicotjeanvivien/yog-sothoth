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

import { ApiClientError } from "@/lib/api/errors";
import { fetchPools, type FetchPoolsParams } from "@/lib/api/pools";
import type { PoolsPage } from "@/lib/api/schemas";

import { PoolsEmptyState } from "@/components/pools/pools-empty-state";
import { PoolsErrorState } from "@/components/pools/pools-error-state";
import { PoolsPagination } from "@/components/pools/pools-pagination";
import { PoolsTable } from "@/components/pools/pools-table";
import type { FormatLocale } from "@/lib/format/date";

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

/**
 * Outcome of the fetch attempt. Surfaces success and failure as a
 * single discriminated value so the JSX branch is symmetric.
 */
type FetchOutcome =
  | { kind: "ok"; page: PoolsPage }
  | { kind: "error"; cause: ApiClientError };

// ── Helpers ───────────────────────────────────────────────────────────

/**
 * Normalise the cursor read from `searchParams`. Next.js types
 * `searchParams[k]` as `string | string[] | undefined` to support
 * repeated keys (`?cursor=a&cursor=b`); for an opaque token, we keep
 * the first occurrence and ignore the rest.
 */
function pickCursor(raw: string | string[] | undefined): string | undefined {
  if (raw === undefined) return undefined;
  const value = Array.isArray(raw) ? raw[0] : raw;
  return value && value.length > 0 ? value : undefined;
}

/** Pick the BFF-style `kind` that best describes the error for the UI. */
function resolveErrorKind(
  err: ApiClientError,
): "timeout" | "unavailable" | "bad_request" | "unexpected" {
  switch (err.details.kind) {
    case "timeout":
      return "timeout";
    case "network":
    case "validation":
      return "unavailable";
    case "http":
      if (err.details.status >= 400 && err.details.status < 500) {
        return "bad_request";
      }
      return "unavailable";
  }
}

// ── Page ──────────────────────────────────────────────────────────────

export default async function PoolsListingPage({
  params,
  searchParams,
}: PoolsPageProps) {
  const { locale } = await params;
  const { cursor: rawCursor } = await searchParams;
  setRequestLocale(locale);

  const tPage = await getTranslations("Pools.page");
  const tTable = await getTranslations("Pools.table");
  const tEmpty = await getTranslations("Pools.empty");
  const tError = await getTranslations("Pools.error");
  const tPagination = await getTranslations("Pools.pagination");

  const cursor = pickCursor(rawCursor);
  const outcome = await safeFetchPools({ ...(cursor !== undefined && { cursor }) });

  const basePath = `/${locale}/pools`;

  return (
    <div className="mx-auto max-w-7xl px-6 py-10 lg:px-10 lg:py-12">
      {/* Page header */}
      <header className="mb-8">
        <p className="text-[10px] uppercase tracking-[0.32em] text-sothoth-400/70">
          {tPage("eyebrow")}
        </p>
        <h1 className="mt-2 font-display text-3xl tracking-wider text-sothoth-400">
          {tPage("title")}
        </h1>
        <p className="mt-2 max-w-2xl text-sm text-slate-400">
          {tPage("description")}
        </p>
      </header>

      {/* Body — branches on outcome */}
      {outcome.kind === "error" ? (
        <PoolsErrorState
          title={tError("title")}
          description={tError(resolveErrorKind(outcome.cause))}
          retryHref={basePath}
          retryLabel={tError("retry")}
        />
      ) : outcome.page.items.length === 0 ? (
        <PoolsEmptyState
          title={tEmpty("title")}
          description={tEmpty("description")}
        />
      ) : (
        <>
          <PoolsTable
            pools={outcome.page.items}
            locale={locale as FormatLocale}
            labels={{
              address: tTable("address"),
              protocol: tTable("protocol"),
              pair: tTable("pair"),
              firstSeen: tTable("firstSeen"),
              lastSeen: tTable("lastSeen"),
            }}
          />
          <PoolsPagination
            basePath={basePath}
            nextCursor={outcome.page.next_cursor}
            labels={{
              next: tPagination("next"),
              firstPage: tPagination("firstPage"),
            }}
          />
        </>
      )}
    </div>
  );
}

// ── Outcome-wrapped fetch ─────────────────────────────────────────────

/**
 * Wrap `fetchPools` so the calling JSX branches on a value rather than
 * a try/catch. Mirrors the "result type" pattern used pervasively on
 * the Rust side (`CoreResult`, `RepositoryResult`).
 */
async function safeFetchPools(params: FetchPoolsParams): Promise<FetchOutcome> {
  try {
    const page = await fetchPools(params);
    return { kind: "ok", page };
  } catch (err) {
    if (err instanceof ApiClientError) {
      // Log full internal detail server-side; the UI shows the typed
      // kind only.
      console.error("[Pools] fetch failed:", err.message, err.details);
      return { kind: "error", cause: err };
    }
    // Programmer error (e.g. RangeError from bad params). Re-throw so
    // Next.js error boundary catches it — it's not a user-facing
    // failure mode.
    throw err;
  }
}

// Required for static rendering of locale-scoped pages. The body is
// `force-dynamic`, so this only registers the supported locales.
export function generateStaticParams() {
  return [{ locale: "en" }, { locale: "fr" }];
}