"use client";

/**
 * Overview page — the latest signals, kept live.
 *
 * Client Component: seeded with the server-fetched newest signals,
 * then kept live by `useSignalStream` (the same SSE machinery as the
 * `/signals` feed — one more subscriber on the API's shared
 * broadcast costs nothing). Renders the top `LATEST_SIGNALS_COUNT` of
 * the merged list with the feed's `SignalCard` in its `compact`
 * density — severity column + pair/time/value only: a control tower
 * scans, `/signals` analyses.
 *
 * The header carries the stream-status dot (a broken stream must not
 * look like a quiet one) and the "see all" link to `/signals`.
 */

import { useTranslations } from "next-intl";

import { SignalCard } from "@/components/dashboard/signals/signal-card";
import {
  LATEST_SIGNALS_COUNT,
  STATUS_DOT,
} from "@/components/dashboard/signals/signal-display";
import { useSignalStream } from "@/components/dashboard/signals/use-signal-stream";
import { Link } from "@/i18n/navigation";
import type { SignalResponse } from "@/lib/api/schema/signal";

export function OverviewLatestSignalsLive({
  initial,
}: {
  initial: readonly SignalResponse[];
}) {
  const t = useTranslations("Dashboard.Overview.latestSignals");
  const tFeed = useTranslations("Dashboard.Signals.feed");
  const { signals, status } = useSignalStream(initial);
  const latest = signals.slice(0, LATEST_SIGNALS_COUNT);

  return (
    <div>
      <div className="mb-4 flex items-center justify-between">
        <h2 className="text-[13px] font-semibold tracking-[0.28em] text-slate-400 uppercase">
          {t("title")}
        </h2>
        <span className="flex items-center gap-3 text-[13px]">
          <span
            title={tFeed(`status.${status}`)}
            className={`h-2 w-2 rounded-full ${STATUS_DOT[status]}`}
          >
            <span className="sr-only">{tFeed(`status.${status}`)}</span>
          </span>
          <Link
            href="/signals"
            className="text-sothoth-200 underline-offset-4 hover:underline"
          >
            {t("seeAll")}
          </Link>
        </span>
      </div>

      {latest.length === 0 ? (
        <p className="rounded-[8px] border border-sothoth-500/15 bg-cosmos-700/40 px-4 py-6 text-[14px] text-slate-400">
          {t("empty")}
        </p>
      ) : (
        <ul className="flex flex-col gap-2">
          {latest.map((signal) => (
            <SignalCard key={signal.id} signal={signal} compact />
          ))}
        </ul>
      )}
    </div>
  );
}
