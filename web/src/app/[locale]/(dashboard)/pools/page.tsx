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
 *   - filled → `PoolsTable` + `<Pagination />`
 *
 * URL state drives pagination: `?cursor=...&dir=next|prev` for
 * cursor-relative navigation, `?position=first|last` for absolute
 * jumps. Invalid combinations are tolerated client-side and
 * surfaced as a 400 from yog-api → rendered as PoolsError.
 *
 * Search, filters and sortable column headers are intentionally
 * out of scope here — each lands in a dedicated commit.
 */

import { setRequestLocale, getTranslations } from "next-intl/server";
import type { Metadata } from "next";

import { PoolsHeader } from "@/components/dashboard/pools/pools-header";
import { PoolsTable } from "@/components/dashboard/pools/pools-table";
import { PoolsEmpty } from "@/components/dashboard/pools/pools-empty";
import { PoolsError } from "@/components/dashboard/pools/pools-error";
import { Pagination } from "@/components/shared/pagination";

import { fetchPools } from "@/lib/api/pools";
import { ApiClientError, type ApiClientErrorKind } from "@/lib/api/errors";
import type {
  PageDir,
  PagePosition,
  PoolSort,
} from "@/lib/api/type/pagination";
import type { PoolsPageResponse } from "@/lib/api/schema/page";
import { PoolsNoResults } from "@/components/dashboard/pools/pool-no-result";

// ── Page metadata ─────────────────────────────────────────────────────

type PoolsPageProps = {
  params: Promise<{ locale: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
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

// ── Search params parsing ─────────────────────────────────────────────
//
// We accept anything as a search param shape and narrow defensively.
// Out-of-vocabulary values (e.g. `dir=sideways`) are silently dropped
// rather than rejected — the URL is user-editable and a stale link
// shouldn't crash the page. yog-api gets a request without the bad
// param and returns a normal first page.

function parseDir(raw: string | string[] | undefined): PageDir | undefined {
  if (raw === "next" || raw === "prev") return raw;
  return undefined;
}

function parsePosition(
  raw: string | string[] | undefined,
): PagePosition | undefined {
  if (raw === "first" || raw === "last") return raw;
  return undefined;
}

function parseCursor(raw: string | string[] | undefined): string | undefined {
  if (typeof raw !== "string") return undefined;
  if (raw.length === 0) return undefined;
  return raw;
}

function parseSearch(raw: string | string[] | undefined): string | undefined {
  if (typeof raw !== "string") return undefined;
  const trimmed = raw.trim();
  return trimmed.length > 0 ? trimmed : undefined;
}

function parseSort(raw: string | string[] | undefined): PoolSort | undefined {
  if (
    raw === "first_seen_asc" ||
    raw === "first_seen_desc" ||
    raw === "last_seen_asc" ||
    raw === "last_seen_desc"
  ) {
    return raw;
  }
  return undefined;
}

// ── Fetch result type ────────────────────────────────────────────────

type FetchOutcome =
  | { kind: "ok"; data: PoolsPageResponse }
  | { kind: "error"; reason: ApiClientErrorKind };

async function load(args: {
  cursor: string | undefined;
  dir: PageDir | undefined;
  position: PagePosition | undefined;
  search: string | undefined;
  sort: PoolSort | undefined;
}): Promise<FetchOutcome> {
  try {
    const data = await fetchPools({
      cursor: args.cursor,
      dir: args.dir,
      position: args.position,
      q: args.search,
      sort: args.sort,
    });
    return { kind: "ok", data };
  } catch (err) {
    if (err instanceof ApiClientError) {
      return { kind: "error", reason: err.details.kind };
    }
    throw err;
  }
}

// ── Page ──────────────────────────────────────────────────────────────

export default async function PoolsPage({
  params,
  searchParams,
}: PoolsPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);

  const sp = await searchParams;
  const cursor = parseCursor(sp["cursor"]);
  const dir = parseDir(sp["dir"]);
  const position = parsePosition(sp["position"]);
  const search = parseSearch(sp["q"]);
  const sort = parseSort(sp["sort"]);

  const outcome = await load({ cursor, dir, position, search, sort });

  // Resolve the effective sort for the header indicators: defaults to
  // first_seen_desc when the URL doesn't specify one (matches the
  // backend default).
  const effectiveSort: PoolSort = sort ?? "first_seen_desc";

  const hasActiveSearch = search !== undefined;

  return (
    <div className="pb-16">
      <PoolsHeader />

      {outcome.kind === "error" ? (
        <PoolsError kind={outcome.reason} />
      ) : outcome.data.items.length === 0 ? (
        hasActiveSearch ? (
          <PoolsNoResults query={search!} />
        ) : (
          <PoolsEmpty />
        )
      ) : (
        <>
          <PoolsTable
            pools={outcome.data.items}
            locale={locale}
            currentSort={effectiveSort}
            searchParams={sp}
          />
          <Pagination
            page={outcome.data}
            searchParams={sp}
            basePath="/pools"
          />
        </>
      )}
    </div>
  );
}