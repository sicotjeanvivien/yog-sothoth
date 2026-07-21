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
 * Renders the *shared* `PoolsTableRow` — same columns and look as `/pools` —
 * so the watchlist is visibly "your subset of the pools list". Removal is the
 * row's own watchlist star (in the actions cell); no bespoke control here.
 * `signalLabels` and `locale` are resolved client-side and passed to the row.
 *
 * States: a pre-hydration skeleton (the store reads "empty" until mounted, so
 * we must not flash the empty state), an empty state with a CTA to `/pools`,
 * and the table. Pools that fail to load (e.g. a 404) are omitted; if any
 * failed a discreet note says so rather than faking a row.
 */

"use client";

import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { useLocale, useTranslations } from "next-intl";

import { KNOWN_DETECTORS } from "@/components/dashboard/signals/signal-display";
import { CtaLink } from "@/components/shared/cta-link";
import { fetchPoolBrowser } from "@/lib/api/browser/pool";
import type { PoolResponse } from "@/lib/api/schema/pool";
import { useWatchlist } from "@/lib/watchlist/use-watchlist";

import { PoolsTableRow } from "@/components/dashboard/pools/pools-table-row";
import {
  GRID_COLS,
  HEAD_CELL_CLASS,
  HEAD_CELL_NUMERIC_CLASS,
  TABLE_MIN_WIDTH_CLASS,
  type SignalCellLabels,
} from "@/components/dashboard/pools/pools-table-shared";

const noopSubscribe = () => () => {};

export function WatchlistContent() {
  const t = useTranslations("Dashboard.Watchlist");
  const tTable = useTranslations("Dashboard.Pools.table");
  const tSignals = useTranslations("Dashboard.Signals.feed");
  const locale = useLocale();
  const { addresses } = useWatchlist();

  // `false` on the server / during hydration, `true` on the client — a
  // mount flag without an effect, so no cascading-render lint.
  const mounted = useSyncExternalStore(
    noopSubscribe,
    () => true,
    () => false,
  );

  const [pools, setPools] = useState<PoolResponse[]>([]);
  const [anyError, setAnyError] = useState(false);
  // The address set the last fetch settled for; while it differs from the
  // current one we're loading. Set only from the async callback.
  const [settledKey, setSettledKey] = useState<string | null>(null);

  const addressesKey = addresses.join(",");
  const latestRequest = useRef(0);

  useEffect(() => {
    if (addresses.length === 0) return;

    const requestId = ++latestRequest.current;
    Promise.allSettled(addresses.map((a) => fetchPoolBrowser(a))).then(
      (results) => {
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
    return isLoading ? (
      <Skeleton label={t("loading")} />
    ) : (
      <Skeleton label={t("partialError")} />
    );
  }

  const signalLabels: SignalCellLabels = {
    tagFor: (detector) =>
      KNOWN_DETECTORS.has(detector)
        ? tSignals(`detectors.${detector}.tag`)
        : detector,
    ariaFor: (count) => tTable("signalsAria", { count }),
    title: tTable("signalsPopoverTitle"),
  };

  return (
    <>
      <div className="mx-6 overflow-x-auto rounded-[8px] border border-sothoth-500/15 bg-cosmos-900/40 lg:mx-10">
        <div role="table" className={TABLE_MIN_WIDTH_CLASS}>
          <div
            role="rowgroup"
            className="border-b border-sothoth-500/20 bg-cosmos-900/60"
          >
            <div role="row" className={`grid ${GRID_COLS}`}>
              <div role="columnheader" className={HEAD_CELL_CLASS}>
                {tTable("pair")}
              </div>
              <div role="columnheader" className={HEAD_CELL_CLASS}>
                {tTable("signals")}
              </div>
              <div role="columnheader" className={HEAD_CELL_CLASS}>
                {tTable("protocol")}
              </div>
              <div role="columnheader" className={HEAD_CELL_NUMERIC_CLASS}>
                {tTable("fee")}
              </div>
              <div role="columnheader" className={HEAD_CELL_NUMERIC_CLASS}>
                {tTable("tvl")}
              </div>
              <div role="columnheader" className={HEAD_CELL_NUMERIC_CLASS}>
                {tTable("volume24h")}
              </div>
              <div role="columnheader" className={HEAD_CELL_CLASS}>
                {tTable("firstSeen")}
              </div>
              <div role="columnheader" className={HEAD_CELL_CLASS}>
                {tTable("lastSeen")}
              </div>
              <div
                role="columnheader"
                className={HEAD_CELL_NUMERIC_CLASS}
                aria-hidden="true"
              />
            </div>
          </div>

          <div role="rowgroup">
            {visible.map((pool) => (
              <PoolsTableRow
                key={pool.poolAddress}
                pool={pool}
                locale={locale}
                signalLabels={signalLabels}
              />
            ))}
          </div>
        </div>
      </div>

      {anyError && (
        <p className="mx-6 mt-3 text-[13px] text-slate-500 lg:mx-10">
          {t("partialError")}
        </p>
      )}
    </>
  );
}

function Skeleton({ label }: { label: string }) {
  return (
    <div className="mx-6 rounded-[8px] border border-sothoth-500/15 bg-cosmos-700/40 px-6 py-12 text-center text-[14px] text-slate-500 lg:mx-10">
      {label}
    </div>
  );
}
