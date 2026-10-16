/**
 * The pools page, a Server Component calling `fetchPools` directly. Error, empty and filled
 * states are exclusive; the URL carries the pagination (`cursor` + `dir`, or `position`).
 */

import { setRequestLocale, getTranslations } from "next-intl/server";
import type { Metadata } from "next";

import { PoolsHeader } from "@/components/dashboard/pools/pools-header";
import { PoolsTable } from "@/components/dashboard/pools/pools-table";
import { PoolsEmpty } from "@/components/dashboard/pools/pools-empty";
import { PoolsError } from "@/components/dashboard/pools/pools-error";
import { PoolsMovedNotice } from "@/components/dashboard/pools/pools-moved-notice";
import { PoolsEmptied } from "@/components/dashboard/pools/pools-emptied";
import { Pagination } from "@/components/shared/pagination";

import { fetchPools } from "@/lib/api/server/pools";
import { ApiClientError, type ApiClientErrorKind } from "@/lib/api/errors";
import type {
  PageDir,
  PagePosition,
  PoolSort,
} from "@/lib/api/type/pagination";
import type { PoolPageResponsePoolResponse } from "@/lib/api/schema/zod.gen";
import { PoolsNoResults } from "@/components/dashboard/pools/pool-no-result";


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

// The URL is user-editable: an out-of-vocabulary value is dropped, and the API gets a first page.

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

// Any well-formed tier goes to the API, an unknown one giving an empty page; a malformed
// one is dropped rather than 400 the page.
function parseFeeBps(raw: string | string[] | undefined): string | undefined {
  if (typeof raw !== "string") return undefined;
  const trimmed = raw.trim();
  if (trimmed.length === 0) return undefined;
  return /^\d+(\.\d+)?$/.test(trimmed) ? trimmed : undefined;
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


type FetchOutcome =
  | { kind: "ok"; data: PoolPageResponsePoolResponse }
  | { kind: "error"; reason: ApiClientErrorKind };

async function load(args: {
  cursor: string | undefined;
  dir: PageDir | undefined;
  position: PagePosition | undefined;
  search: string | undefined;
  sort: PoolSort | undefined;
  feeBps: string | undefined;
}): Promise<FetchOutcome> {
  try {
    const data = await fetchPools({
      cursor: args.cursor,
      dir: args.dir,
      position: args.position,
      q: args.search,
      sort: args.sort,
      feeBps: args.feeBps,
    });
    return { kind: "ok", data };
  } catch (err) {
    if (err instanceof ApiClientError) {
      return { kind: "error", reason: err.details.kind };
    }
    throw err;
  }
}


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
  const feeBps = parseFeeBps(sp["fee_bps"]);
  const outcome = await load({ cursor, dir, position, search, sort, feeBps });

  // ⚠️ The backend's default sort: the two must stay in step.
  const effectiveSort: PoolSort = sort ?? "last_seen_desc";

  const hasActiveRefinement = search !== undefined || feeBps !== undefined;

  // Empty mid-traversal: every remaining pool moved out. Neither "nothing indexed" nor "no
  // match" — the moved notice is the only way out, as an empty page carries no cursor.
  const emptied =
    outcome.kind === "ok" &&
    outcome.data.items.length === 0 &&
    outcome.data.touchedSince > 0 &&
    outcome.data.asOf !== null;

  const movedNotice =
    outcome.kind === "ok" &&
    outcome.data.touchedSince > 0 &&
    outcome.data.asOf !== null ? (
      <PoolsMovedNotice
        count={outcome.data.touchedSince}
        asOf={outcome.data.asOf}
        sort={effectiveSort}
        locale={locale}
        searchParams={sp}
      />
    ) : null;

  return (
    <div className="pb-16">
      <PoolsHeader />

      {outcome.kind === "error" ? (
        <PoolsError kind={outcome.reason} />
      ) : emptied ? (
        <>
          {movedNotice}
          <PoolsEmptied />
        </>
      ) : outcome.data.items.length === 0 ? (
        hasActiveRefinement ? (
          <PoolsNoResults query={search} />
        ) : (
          <PoolsEmpty />
        )
      ) : (
        <>
          {movedNotice}
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