/**
 * Pool detail page (`/[locale]/(dashboard)/pools/[address]`).
 *
 * Server Component. Orchestrates four parallel API calls, gates the
 * page on the critical one, and degrades gracefully on the rest:
 *
 *   - `fetchPool`             → CRITICAL. 404 → notFound(). Other
 *                                errors → full page error UI.
 *   - `fetchPoolLatestState`  → non-critical, 404 is expected for
 *                                pools observed only through
 *                                Claim* events. null on any error
 *                                or 404, the KPI block adapts.
 *   - `fetchPoolSwapEvents`        → non-critical. Block-level error
 *                                substituted in place of the table.
 *   - `fetchPoolLiquidityEvents` → same as swaps.
 *
 * The block order is Header → KPIs → Info → Swaps → Liquidity,
 * with each block self-contained so the order can change in one
 * place if we ever need to. Swaps and Liquidity blocks already
 * carry their own outer `<section>` with padding, so the page
 * only wraps the error fallback in a matching section to keep
 * layout consistent on failure.
 *
 * Pagination state for the two paginated blocks (swaps, liquidity)
 * lives in the URL behind namespaced params: `swapsCursor` / `swapsDir`
 * / `swapsPosition` and `liqCursor` / `liqDir` / `liqPosition`.
 * The two paginations are fully independent — paginating swaps
 * preserves the liquidity cursor and vice versa.
 */

import { setRequestLocale, getTranslations } from "next-intl/server";
import { notFound } from "next/navigation";
import type { Metadata } from "next";

import { fetchPool } from "@/lib/api/server/pool";
import { fetchPoolLatestState } from "@/lib/api/server/latest-state";
import { fetchPoolSwapEvents } from "@/lib/api/server/swap-events";
import { fetchPoolLiquidityEvents } from "@/lib/api/server/liquidity-events";
import { safeFetch, safeFetchOrNotFound } from "@/lib/api/safe-fetch";
import type {
  PageDir,
  PagePosition,
} from "@/lib/api/type/pagination";

import { PoolDetailHeader } from "@/components/dashboard/pool-detail/pool-detail-header";
import { PoolDetailKpis } from "@/components/dashboard/pool-detail/pool-detail-kpis";
import { PoolDetailInfo } from "@/components/dashboard/pool-detail/pool-detail-info";
import { PoolDetailSwaps } from "@/components/dashboard/pool-detail/pool-detail-swaps";
import { PoolDetailLiquidity } from "@/components/dashboard/pool-detail/pool-detail-liquidity";
import { BlockError } from "@/components/dashboard/block-error";
import { PageError } from "@/components/dashboard/page-error";
import { Pagination } from "@/components/shared/pagination";

// ── Page metadata ─────────────────────────────────────────────────────

type PoolDetailPageProps = {
  params: Promise<{ locale: string; address: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
};

export async function generateMetadata({
  params,
}: PoolDetailPageProps): Promise<Metadata> {
  const { locale, address } = await params;
  const t = await getTranslations({
    locale,
    namespace: "Dashboard.PoolDetail.meta",
  });

  // We deliberately don't fetch the pool here to enrich the title
  // with the pair symbols — that would mean a redundant API call
  // just for metadata. The short address in the title is enough
  // to disambiguate tabs.
  const shortAddress = `${address.slice(0, 4)}...${address.slice(-4)}`;
  return {
    title: `${t("title", { address: shortAddress })} — Yog-Scope`,
    description: t("description"),
  };
}

// ── Search params parsing ─────────────────────────────────────────────
//
// Accept anything and narrow defensively. Out-of-vocabulary values
// (e.g. `swapsDir=sideways`) are silently dropped rather than
// rejected — the URL is user-editable and a stale link shouldn't
// crash the page.

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

// ── Page ──────────────────────────────────────────────────────────────

export default async function PoolDetailPage({
  params,
  searchParams,
}: PoolDetailPageProps) {
  const { locale, address } = await params;
  setRequestLocale(locale);

  // Critical fetch first. Firing the four calls in parallel and
  // checking the pool outcome afterwards would waste bandwidth on
  // the secondary calls when the pool doesn't exist — one extra
  // round-trip of latency in exchange for predictable behaviour.
  const poolOutcome = await safeFetchOrNotFound(() => fetchPool(address));

  if (poolOutcome.kind === "not_found") {
    notFound();
  }

  if (poolOutcome.kind === "error") {
    return <PageError kind={poolOutcome.reason} />;
  }

  const pool = poolOutcome.data;

  // Pagination params, one namespace per block.
  const sp = await searchParams;
  const swapsPagination = {
    cursor: parseCursor(sp['swapsCursor']),
    dir: parseDir(sp['swapsDir']),
    position: parsePosition(sp['swapsPosition']),
    limit: 20,
  };
  const liqPagination = {
    cursor: parseCursor(sp['liqCursor']),
    dir: parseDir(sp['liqDir']),
    position: parsePosition(sp['liqPosition']),
    limit: 20,
  };

  // Three non-critical fetches in parallel. Failures are isolated
  // and rendered as block-level error states.
  const [stateOutcome, swapsOutcome, liquidityOutcome] = await Promise.all([
    safeFetchOrNotFound(() => fetchPoolLatestState(address)),
    safeFetch(() => fetchPoolSwapEvents(address, swapsPagination)),
    safeFetch(() => fetchPoolLiquidityEvents(address, liqPagination)),
  ]);

  // "Latest state" 404 is expected (pool observed via Claim*
  // events only) — collapse it to null so the KPI block adapts.
  // Any other error also collapses to null: TVL + 24h volume stay
  // visible, the composition card simply doesn't render.
  const state = stateOutcome.kind === "ok" ? stateOutcome.data : null;

  const tSwaps = await getTranslations("Dashboard.PoolDetail.swaps");
  const tLiquidity = await getTranslations("Dashboard.PoolDetail.liquidity");

  const basePath = `/pools/${address}`;

  return (
    <div className="pb-16">
      <PoolDetailHeader pool={pool} />

      <PoolDetailKpis pool={pool} state={state} />

      <PoolDetailInfo pool={pool} locale={locale} />

      {swapsOutcome.kind === "ok" ? (
        <>
          <PoolDetailSwaps
            pool={pool}
            swaps={swapsOutcome.data.items}
            locale={locale}
          />
          {swapsOutcome.data.items.length > 0 && (
            <Pagination
              page={swapsOutcome.data}
              searchParams={sp}
              paramPrefix="swaps"
              basePath={basePath}
            />
          )}
        </>
      ) : (
        <section className="mt-6 px-6 lg:px-10">
          <BlockError title={tSwaps("title")} kind={swapsOutcome.reason} />
        </section>
      )}

      {liquidityOutcome.kind === "ok" ? (
        <>
          <PoolDetailLiquidity
            pool={pool}
            events={liquidityOutcome.data.items}
            locale={locale}
          />
          {liquidityOutcome.data.items.length > 0 && (
            <Pagination
              page={liquidityOutcome.data}
              searchParams={sp}
              paramPrefix="liq"
              basePath={basePath}
            />
          )}
        </>
      ) : (
        <section className="mt-6 px-6 lg:px-10">
          <BlockError
            title={tLiquidity("title")}
            kind={liquidityOutcome.reason}
          />
        </section>
      )}
    </div>
  );
}