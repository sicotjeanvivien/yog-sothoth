/**
 * The watched pools, fetched from the browser and rendered with the shared `PoolsTableRow`.
 * ⚠️ A skeleton until mounted: the store reads empty before hydration, which must not flash
 * the empty state. A pool that fails to load is omitted, and a note says so.
 */

"use client";

import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { useLocale, useTranslations } from "next-intl";

import { KNOWN_DETECTORS } from "@/components/dashboard/signals/signal-display";
import { CtaLink } from "@/components/shared/cta-link";
import { fetchPoolBrowser } from "@/lib/api/browser/pool";
import type { PoolResponse } from "@/lib/api/schema/zod.gen";
import { useWatchlist } from "@/lib/watchlist/use-watchlist";

import { PoolsTableRow } from "@/components/dashboard/pools/pools-table-row";
import {
  GRID_COLS,
  HEAD_CELL_CLASS,
  HEAD_CELL_NUMERIC_CLASS,
  TABLE_MIN_WIDTH_CLASS,
  type CoverageLabel,
  type SignalCellLabels,
} from "@/components/dashboard/pools/pools-table-shared";

const noopSubscribe = () => () => {};

export function WatchlistContent() {
  const t = useTranslations("Dashboard.Watchlist");
  const tTable = useTranslations("Dashboard.Pools.table");
  const tSignals = useTranslations("Dashboard.Signals.feed");
  const locale = useLocale();
  const { addresses } = useWatchlist();

  // A mount flag without an effect: no cascading render.
  const mounted = useSyncExternalStore(
    noopSubscribe,
    () => true,
    () => false,
  );

  const [pools, setPools] = useState<PoolResponse[]>([]);
  const [anyError, setAnyError] = useState(false);
  // Loading while it differs from the current set; set only from the async callback.
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

  // From the live set, so a removal shows before the refetch settles.
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

  const coverageLabel: CoverageLabel = (priced, total) =>
    tTable("volumeCoverage", { priced, total });

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
                coverageLabel={coverageLabel}
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
