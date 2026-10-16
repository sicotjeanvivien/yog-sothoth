/**
 * The pool detail page. The pool is fetched first and gates the page (404 → `notFound()`);
 * the latest state and the active tab's data follow, each degrading on its own. Each
 * paginated block keeps its cursor in its own URL params (`swaps…`, `liq…`, `alerts…`).
 */

import { setRequestLocale, getTranslations } from "next-intl/server";
import { notFound } from "next/navigation";
import type { Metadata } from "next";

import { fetchPool } from "@/lib/api/server/pool";
import { fetchPoolLatestState } from "@/lib/api/server/latest-state";
import { fetchPoolSwapEvents } from "@/lib/api/server/swap-events";
import { fetchPoolLiquidityEvents } from "@/lib/api/server/liquidity-events";
import { fetchPoolHistory } from "@/lib/api/server/pool-history";
import { fetchSignals } from "@/lib/api/server/signals";
import { safeFetch, safeFetchOrNotFound } from "@/lib/api/safe-fetch";
import type {
  PageDir,
  PagePosition,
} from "@/lib/api/type/pagination";

import { PoolDetailHeader } from "@/components/dashboard/pool-detail/pool-detail-header";
import { PoolDetailKpis } from "@/components/dashboard/pool-detail/pool-detail-kpis";
import { PoolDetailInfo } from "@/components/dashboard/pool-detail/pool-detail-info";
import { PoolDetailFees } from "@/components/dashboard/pool-detail/pool-detail-fees";
import {
  PoolDetailTabs,
  parseTab,
} from "@/components/dashboard/pool-detail/pool-detail-tabs";
import { PoolDetailSwaps } from "@/components/dashboard/pool-detail/pool-detail-swaps";
import { PoolDetailLiquidity } from "@/components/dashboard/pool-detail/pool-detail-liquidity";
import { PoolDetailAlerts } from "@/components/dashboard/pool-detail/pool-detail-alerts";
import { BlockError } from "@/components/dashboard/block-error";
import { PageError } from "@/components/dashboard/page-error";
import { Pagination } from "@/components/shared/pagination";

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

  // No pool fetch for the pair symbols: an API call for a title.
  const shortAddress = `${address.slice(0, 4)}...${address.slice(-4)}`;
  return {
    title: `${t("title", { address: shortAddress })} — Yog-Scope`,
    description: t("description"),
  };
}

// The URL is user-editable: an out-of-vocabulary value is dropped, never an error.

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

const HISTORY_DAYS = 30;

export default async function PoolDetailPage({
  params,
  searchParams,
}: PoolDetailPageProps) {
  const { locale, address } = await params;
  setRequestLocale(locale);

  // The pool first: the other calls would be wasted on a pool that does not exist.
  const poolOutcome = await safeFetchOrNotFound(() => fetchPool(address));

  if (poolOutcome.kind === "not_found") {
    notFound();
  }

  if (poolOutcome.kind === "error") {
    return <PageError kind={poolOutcome.reason} />;
  }

  const pool = poolOutcome.data;

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
  const alertsPagination = {
    cursor: parseCursor(sp['alertsCursor']),
    dir: parseDir(sp['alertsDir']),
    position: parsePosition(sp['alertsPosition']),
    limit: 20,
  };

  const tab = parseTab(sp["tab"]);

  // KPIs (state) are always shown above the tabs; only the active tab's data
  // is fetched, in parallel with the state. Inactive tabs resolve to `null`.
  const [
    stateOutcome,
    swapsOutcome,
    liquidityOutcome,
    historyOutcome,
    alertsOutcome,
  ] = await Promise.all([
    safeFetchOrNotFound(() => fetchPoolLatestState(address)),
    tab === "swaps"
      ? safeFetch(() => fetchPoolSwapEvents(address, swapsPagination))
      : Promise.resolve(null),
    tab === "liquidity"
      ? safeFetch(() => fetchPoolLiquidityEvents(address, liqPagination))
      : Promise.resolve(null),
    tab === "fees"
      ? safeFetch(() => fetchPoolHistory(address, HISTORY_DAYS))
      : Promise.resolve(null),
    tab === "alerts"
      ? safeFetch(() => fetchSignals({ pool: address, ...alertsPagination }))
      : Promise.resolve(null),
  ]);

  // Any failure, the expected 404 included, is null: the KPI strip adapts.
  const state = stateOutcome.kind === "ok" ? stateOutcome.data : null;

  const tFees = await getTranslations("Dashboard.PoolDetail.fees");
  const tSwaps = await getTranslations("Dashboard.PoolDetail.swaps");
  const tLiquidity = await getTranslations("Dashboard.PoolDetail.liquidity");
  const tAlerts = await getTranslations("Dashboard.PoolDetail.alerts");

  const basePath = `/pools/${address}`;

  return (
    <div className="pb-16">
      <PoolDetailHeader pool={pool} />

      <PoolDetailKpis pool={pool} state={state} />

      <PoolDetailTabs basePath={basePath} activeTab={tab} />

      {tab === "info" && <PoolDetailInfo pool={pool} locale={locale} />}

      {tab === "fees" &&
        historyOutcome !== null &&
        (historyOutcome.kind === "ok" ? (
          <PoolDetailFees history={historyOutcome.data} locale={locale} />
        ) : (
          <section className="mt-6 px-6 lg:px-10">
            <BlockError title={tFees("title")} kind={historyOutcome.reason} />
          </section>
        ))}

      {tab === "swaps" &&
        swapsOutcome !== null &&
        (swapsOutcome.kind === "ok" ? (
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
        ))}

      {tab === "liquidity" &&
        liquidityOutcome !== null &&
        (liquidityOutcome.kind === "ok" ? (
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
        ))}

      {tab === "alerts" &&
        alertsOutcome !== null &&
        (alertsOutcome.kind === "ok" ? (
          <>
            <PoolDetailAlerts signals={alertsOutcome.data.items} />
            {alertsOutcome.data.items.length > 0 && (
              <Pagination
                page={alertsOutcome.data}
                searchParams={sp}
                paramPrefix="alerts"
                basePath={basePath}
              />
            )}
          </>
        ) : (
          <section className="mt-6 px-6 lg:px-10">
            <BlockError title={tAlerts("title")} kind={alertsOutcome.reason} />
          </section>
        ))}
    </div>
  );
}