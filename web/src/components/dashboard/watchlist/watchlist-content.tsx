/**
 * Watchlist content — the client-rendered list of watched pools.
 *
 * Reads the pool addresses from the LocalStorage watchlist (`useWatchlist`)
 * and fetches each pool straight from the browser (`fetchPoolBrowser`) — no
 * server round-trip and no batch endpoint, which a small personal watchlist
 * doesn't warrant. When the list changes (a star toggled here or on a pool
 * page, in this tab or another) it reconciles: removed pools disappear
 * immediately, added ones are fetched.
 *
 * States: a pre-hydration skeleton (the store reads "empty" until mounted, so
 * we must not flash the empty state), an empty state with a CTA to `/pools`,
 * and the table. Pools that fail to load (e.g. a 404) are simply omitted; if
 * any failed a discreet note says so rather than faking a row.
 */

"use client";

import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { useTranslations } from "next-intl";

import { CtaLink } from "@/components/shared/cta-link";
import { WatchlistIcon } from "@/components/shared/icon";
import { Link } from "@/i18n/navigation";
import { fetchPoolBrowser } from "@/lib/api/browser/pool";
import type { PoolResponse } from "@/lib/api/schema/pool";
import { formatUsdCompact } from "@/lib/format/format-usd";
import { useWatchlist } from "@/lib/watchlist/use-watchlist";

import { PoolPairCell } from "@/components/dashboard/pools/pool-pair-cell";

const GRID_COLS = "grid-cols-[1fr_auto_auto_auto]";
const CELL = "px-4 py-3 text-[14px] flex items-center";
const CELL_NUM = `${CELL} justify-end font-mono text-slate-300`;
const HEAD =
  "px-4 py-3 text-[12px] font-semibold tracking-[0.2em] text-slate-500 uppercase flex items-center";
const HEAD_NUM = `${HEAD} justify-end`;

// `useSyncExternalStore` mount flag: `false` on the server / during hydration,
// `true` once running on the client — no effect, so no cascading-render lint.
const noopSubscribe = () => () => {};

export function WatchlistContent() {
  const t = useTranslations("Dashboard.Watchlist");
  const { addresses, remove } = useWatchlist();

  const mounted = useSyncExternalStore(
    noopSubscribe,
    () => true,
    () => false,
  );

  const [pools, setPools] = useState<PoolResponse[]>([]);
  const [anyError, setAnyError] = useState(false);
  // The address set the last fetch settled for; while it differs from the
  // current one we're loading. Set only from the async callback (never
  // synchronously in the effect) to avoid cascading renders.
  const [settledKey, setSettledKey] = useState<string | null>(null);

  const addressesKey = addresses.join(",");
  const latestRequest = useRef(0);

  useEffect(() => {
    if (addresses.length === 0) return;

    const requestId = ++latestRequest.current;
    Promise.allSettled(addresses.map((a) => fetchPoolBrowser(a))).then(
      (results) => {
        // Drop a stale response if the address set changed meanwhile.
        if (requestId !== latestRequest.current) return;
        setPools(
          results
            .filter(
              (r): r is PromiseFulfilledResult<PoolResponse> =>
                r.status === "fulfilled",
            )
            .map((r) => r.value),
        );
        setAnyError(results.some((r) => r.status === "rejected"));
        setSettledKey(addressesKey);
      },
    );
  }, [addressesKey, addresses]);

  if (!mounted) {
    return <Skeleton label={t("loading")} />;
  }

  if (addresses.length === 0) {
    return (
      <div className="mx-6 rounded-[8px] border border-sothoth-500/15 bg-cosmos-700/40 px-6 py-12 text-center lg:mx-10">
        <p className="text-[15px] text-slate-300">{t("empty.title")}</p>
        <p className="mt-2 text-[14px] text-slate-500">{t("empty.hint")}</p>
        <div className="my-5">
          <CtaLink href="/pools" label={t("empty.cta")} />
        </div>
      </div>
    );
  }

  // Render from the live address set so a removal reflects instantly, even
  // before the refetch settles.
  const byAddress = new Map(pools.map((p) => [p.poolAddress, p]));
  const visible = addresses
    .map((a) => byAddress.get(a))
    .filter((p): p is PoolResponse => p !== undefined);

  const isLoading = settledKey !== addressesKey;

  if (visible.length === 0) {
    // Still fetching this set, or every watched pool failed to load.
    return isLoading ? (
      <Skeleton label={t("loading")} />
    ) : (
      <Skeleton label={t("partialError")} />
    );
  }

  return (
    <div className="mx-6 lg:mx-10">
      <div
        role="table"
        className="overflow-hidden rounded-[8px] border border-sothoth-500/15 bg-cosmos-700/40"
      >
        <div
          role="row"
          className={`grid ${GRID_COLS} border-b border-sothoth-500/15`}
        >
          <div role="columnheader" className={HEAD}>
            {t("table.pair")}
          </div>
          <div role="columnheader" className={HEAD_NUM}>
            {t("table.tvl")}
          </div>
          <div role="columnheader" className={HEAD_NUM}>
            {t("table.volume24h")}
          </div>
          <div role="columnheader" className={HEAD} aria-hidden="true" />
        </div>

        {visible.map((pool) => (
          <div
            key={pool.poolAddress}
            role="row"
            className={`grid ${GRID_COLS} border-b border-sothoth-500/10 transition-colors last:border-b-0 hover:bg-sothoth-500/[0.04]`}
          >
            {/* The clickable cells are Links; the remove button is a sibling,
                never nested inside a Link. */}
            <Link
              role="cell"
              href={`/pools/${pool.poolAddress}`}
              className={`${CELL} min-w-0`}
            >
              <PoolPairCell tokenA={pool.tokenA} tokenB={pool.tokenB} />
            </Link>
            <Link
              role="cell"
              href={`/pools/${pool.poolAddress}`}
              className={CELL_NUM}
            >
              {formatUsdCompact(pool.tvlUsd)}
            </Link>
            <Link
              role="cell"
              href={`/pools/${pool.poolAddress}`}
              className={CELL_NUM}
            >
              {formatUsdCompact(pool.volume24hUsd)}
            </Link>
            <div role="cell" className={`${CELL} justify-end`}>
              <button
                type="button"
                onClick={() => remove(pool.poolAddress)}
                aria-label={t("removeLabel")}
                className="inline-flex items-center rounded p-1 text-sothoth-300 transition-colors hover:text-sothoth-100 focus-visible:ring-2 focus-visible:ring-sothoth-400 focus-visible:outline-none"
              >
                <WatchlistIcon size={16} className="fill-current" />
              </button>
            </div>
          </div>
        ))}
      </div>

      {anyError && (
        <p className="mt-3 text-[13px] text-slate-500">{t("partialError")}</p>
      )}
    </div>
  );
}

function Skeleton({ label }: { label: string }) {
  return (
    <div className="mx-6 rounded-[8px] border border-sothoth-500/15 bg-cosmos-700/40 px-6 py-12 text-center text-[14px] text-slate-500 lg:mx-10">
      {label}
    </div>
  );
}
