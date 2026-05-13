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

import { notFound } from "next/navigation";
import { getTranslations, setRequestLocale } from "next-intl/server";

import { ApiClientError } from "@/lib/api/errors";
import { fetchPool } from "@/lib/api/pool";
import {
  fetchPoolLatestState,
} from "@/lib/api/latest-state";
import {
  fetchPoolSwaps,
  type FetchPoolSwapsParams,
} from "@/lib/api/swaps";
import {
  fetchPoolLiquidityEvents,
  type FetchPoolLiquidityEventsParams,
} from "@/lib/api/liquidity-events";
import type {
  LiquidityEventsPage,
  PoolCurrentStateResponse,
  PoolResponse,
  SwapEventsPage,
} from "@/lib/api/schemas";

import { FeedEmptyState, FeedErrorState } from "@/components/pool-detail/feed-states";
import { FeedPagination } from "@/components/pool-detail/feed-pagination";
import { FeedSection } from "@/components/pool-detail/feed-section";
import {
  LatestStateCard,
  LatestStateEmpty,
} from "@/components/pool-detail/latest-state-card";
import { LiquidityTable } from "@/components/pool-detail/liquidity-table";
import { PoolHeader } from "@/components/pool-detail/pool-header";
import { SwapsTable } from "@/components/pool-detail/swaps-table";
import type { FormatLocale } from "@/lib/format/date";

// ── Route configuration ────────────────────────────────────────────────

export const dynamic = "force-dynamic";

// ── Types ─────────────────────────────────────────────────────────────

type PoolDetailPageProps = {
  params: Promise<{ locale: string; address: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
};

/**
 * Outcomes for the three fan-out fetches. The latest-state result
 * carries a discriminant on `not_observed_yet` because a 404 there is
 * a meaningful state ("no swap or liquidity event yet"), not a hard
 * error.
 */
type StateOutcome =
  | { kind: "ok"; state: PoolCurrentStateResponse }
  | { kind: "not_observed_yet" }
  | { kind: "error"; cause: ApiClientError };

type SwapsOutcome =
  | { kind: "ok"; page: SwapEventsPage }
  | { kind: "error"; cause: ApiClientError };

type LiquidityOutcome =
  | { kind: "ok"; page: LiquidityEventsPage }
  | { kind: "error"; cause: ApiClientError };

// Cursor query parameter names — two paginations on the same page,
// each one keyed independently.
const SWAPS_CURSOR_KEY = "swaps_cursor";
const LIQUIDITY_CURSOR_KEY = "liquidity_cursor";

// ── Helpers ───────────────────────────────────────────────────────────

function pickFirst(raw: string | string[] | undefined): string | undefined {
  if (raw === undefined) return undefined;
  const value = Array.isArray(raw) ? raw[0] : raw;
  return value && value.length > 0 ? value : undefined;
}

/** Same kind-resolution pattern as the pools listing page. */
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

/**
 * Build a URLSearchParams snapshot for pagination links. The
 * pagination component preserves these and overrides only its own
 * cursor key.
 */
function buildPreservedParams(
  search: Record<string, string | string[] | undefined>,
): URLSearchParams {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(search)) {
    const first = pickFirst(value);
    if (first !== undefined) {
      params.set(key, first);
    }
  }
  return params;
}

// ── Page ──────────────────────────────────────────────────────────────

export default async function PoolDetailPage({
  params,
  searchParams,
}: PoolDetailPageProps) {
  const { locale, address } = await params;
  const search = await searchParams;
  setRequestLocale(locale);

  // ── Step 1: pool identity (sequential). 404 -> notFound() ────────────
  let pool: PoolResponse;
  try {
    pool = await fetchPool(address);
  } catch (err) {
    if (err instanceof ApiClientError) {
      if (err.details.kind === "http" && err.details.status === 404) {
        notFound();
      }
      // Other failure modes (timeout, network, 5xx): re-throw so the
      // error.tsx boundary catches it. We intentionally don't render a
      // partial page when the identity fetch itself fails — without
      // the pool, the rest of the layout has no anchor.
      console.error("[PoolDetail] pool fetch failed:", err.message, err.details);
      throw err;
    }
    throw err;
  }

  // ── Step 2: fan out the three remaining fetches in parallel ─────────
  const swapsCursor = pickFirst(search[SWAPS_CURSOR_KEY]);
  const liquidityCursor = pickFirst(search[LIQUIDITY_CURSOR_KEY]);

  const swapsParams: FetchPoolSwapsParams = {};
  if (swapsCursor !== undefined) swapsParams.cursor = swapsCursor;

  const liquidityParams: FetchPoolLiquidityEventsParams = {};
  if (liquidityCursor !== undefined) liquidityParams.cursor = liquidityCursor;

  const [stateOutcome, swapsOutcome, liquidityOutcome] = await Promise.all([
    safeFetchLatestState(address),
    safeFetchSwaps(address, swapsParams),
    safeFetchLiquidity(address, liquidityParams),
  ]);

  // ── Translations ─────────────────────────────────────────────────────
  const tDetail = await getTranslations("PoolDetail");
  const tDetailHeader = await getTranslations("PoolDetail.header");
  const tDetailState = await getTranslations("PoolDetail.state");
  const tDetailStateEmpty = await getTranslations("PoolDetail.stateEmpty");
  const tDetailSwaps = await getTranslations("PoolDetail.swaps");
  const tDetailLiquidity = await getTranslations("PoolDetail.liquidity");
  const tFeedEmpty = await getTranslations("PoolDetail.feedEmpty");
  const tFeedError = await getTranslations("PoolDetail.feedError");
  const tPagination = await getTranslations("Pools.pagination");

  const basePath = `/${locale}/pools/${address}`;
  const preservedParams = buildPreservedParams(search);

  return (
    <div className="mx-auto max-w-7xl px-6 py-10 lg:px-10 lg:py-12">
      {/* Breadcrumb back to listing */}
      <nav className="mb-6">
        <a
          href={`/${locale}/pools`}
          className="inline-flex items-center text-xs uppercase tracking-[0.18em] text-slate-500 transition-colors hover:text-sothoth-400"
        >
          ← {tDetail("backToList")}
        </a>
      </nav>

      {/* Pool header */}
      <PoolHeader
        pool={pool}
        locale={locale as FormatLocale}
        labels={{
          protocol: tDetailHeader("protocol"),
          pair: tDetailHeader("pair"),
          firstSeen: tDetailHeader("firstSeen"),
          lastSeen: tDetailHeader("lastSeen"),
        }}
      />

      {/* Latest state */}
      <div className="mt-6">
        {stateOutcome.kind === "ok" ? (
          <LatestStateCard
            state={stateOutcome.state}
            locale={locale as FormatLocale}
            labels={{
              sectionTitle: tDetailState("title"),
              reserveA: tDetailState("reserveA"),
              reserveB: tDetailState("reserveB"),
              lastEvent: tDetailState("lastEvent"),
              lastEventKindSwap: tDetailState("kindSwap"),
              lastEventKindLiquidityAdd: tDetailState("kindLiquidityAdd"),
              lastEventKindLiquidityRemove: tDetailState("kindLiquidityRemove"),
              sqrtPrice: tDetailState("sqrtPrice"),
              liquidity: tDetailState("liquidity"),
              signature: tDetailState("signature"),
              updatedAt: tDetailState("updatedAt"),
              notObservedYet: tDetailStateEmpty("title"),
            }}
          />
        ) : stateOutcome.kind === "not_observed_yet" ? (
          <LatestStateEmpty
            title={tDetailStateEmpty("title")}
            description={tDetailStateEmpty("description")}
          />
        ) : (
          <LatestStateEmpty
            title={tDetailState("errorTitle")}
            description={tFeedError(resolveErrorKind(stateOutcome.cause))}
          />
        )}
      </div>

      {/* Swaps feed */}
      <div className="mt-6">
        <FeedSection title={tDetailSwaps("title")}>
          {swapsOutcome.kind === "error" ? (
            <FeedErrorState
              description={tFeedError(resolveErrorKind(swapsOutcome.cause))}
            />
          ) : swapsOutcome.page.items.length === 0 ? (
            <FeedEmptyState description={tFeedEmpty("noSwaps")} />
          ) : (
            <>
              <SwapsTable
                swaps={swapsOutcome.page.items}
                locale={locale as FormatLocale}
                labels={{
                  time: tDetailSwaps("time"),
                  direction: tDetailSwaps("direction"),
                  amountIn: tDetailSwaps("amountIn"),
                  amountOut: tDetailSwaps("amountOut"),
                  fee: tDetailSwaps("fee"),
                  signature: tDetailSwaps("signature"),
                  directionAtoB: tDetailSwaps("directionAtoB"),
                  directionBtoA: tDetailSwaps("directionBtoA"),
                }}
              />
              <FeedPagination
                basePath={basePath}
                cursorKey={SWAPS_CURSOR_KEY}
                nextCursor={swapsOutcome.page.next_cursor}
                preservedParams={preservedParams}
                labels={{
                  next: tPagination("next"),
                  firstPage: tPagination("firstPage"),
                }}
              />
            </>
          )}
        </FeedSection>
      </div>

      {/* Liquidity feed */}
      <div className="mt-6">
        <FeedSection title={tDetailLiquidity("title")}>
          {liquidityOutcome.kind === "error" ? (
            <FeedErrorState
              description={tFeedError(resolveErrorKind(liquidityOutcome.cause))}
            />
          ) : liquidityOutcome.page.items.length === 0 ? (
            <FeedEmptyState description={tFeedEmpty("noLiquidity")} />
          ) : (
            <>
              <LiquidityTable
                events={liquidityOutcome.page.items}
                locale={locale as FormatLocale}
                labels={{
                  time: tDetailLiquidity("time"),
                  kind: tDetailLiquidity("kind"),
                  amountA: tDetailLiquidity("amountA"),
                  amountB: tDetailLiquidity("amountB"),
                  owner: tDetailLiquidity("owner"),
                  signature: tDetailLiquidity("signature"),
                  kindAdd: tDetailLiquidity("kindAdd"),
                  kindRemove: tDetailLiquidity("kindRemove"),
                }}
              />
              <FeedPagination
                basePath={basePath}
                cursorKey={LIQUIDITY_CURSOR_KEY}
                nextCursor={liquidityOutcome.page.next_cursor}
                preservedParams={preservedParams}
                labels={{
                  next: tPagination("next"),
                  firstPage: tPagination("firstPage"),
                }}
              />
            </>
          )}
        </FeedSection>
      </div>
    </div>
  );
}

// ── Outcome-wrapped fetches ───────────────────────────────────────────

async function safeFetchLatestState(address: string): Promise<StateOutcome> {
  try {
    const state = await fetchPoolLatestState(address);
    return { kind: "ok", state };
  } catch (err) {
    if (err instanceof ApiClientError) {
      if (err.details.kind === "http" && err.details.status === 404) {
        return { kind: "not_observed_yet" };
      }
      console.error("[PoolDetail] latest-state failed:", err.message, err.details);
      return { kind: "error", cause: err };
    }
    throw err;
  }
}

async function safeFetchSwaps(
  address: string,
  params: FetchPoolSwapsParams,
): Promise<SwapsOutcome> {
  try {
    const page = await fetchPoolSwaps(address, params);
    return { kind: "ok", page };
  } catch (err) {
    if (err instanceof ApiClientError) {
      console.error("[PoolDetail] swaps failed:", err.message, err.details);
      return { kind: "error", cause: err };
    }
    throw err;
  }
}

async function safeFetchLiquidity(
  address: string,
  params: FetchPoolLiquidityEventsParams,
): Promise<LiquidityOutcome> {
  try {
    const page = await fetchPoolLiquidityEvents(address, params);
    return { kind: "ok", page };
  } catch (err) {
    if (err instanceof ApiClientError) {
      console.error("[PoolDetail] liquidity failed:", err.message, err.details);
      return { kind: "error", cause: err };
    }
    throw err;
  }
}

// Required for static rendering of locale-scoped pages. Body is
// dynamic, so this only registers the supported locales.
export function generateStaticParams() {
  return [{ locale: "en" }, { locale: "fr" }];
}