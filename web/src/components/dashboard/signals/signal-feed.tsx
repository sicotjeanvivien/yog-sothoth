"use client";

/**
 * Live signal feed — the visible end of the Signal Engine loop.
 *
 * Client Component: seeded with the server-rendered first page, then
 * kept live by `useSignalStream` (SSE, direct to the public gateway).
 * New signals prepend in place; the connection state is surfaced as a
 * small status dot so a broken stream is never mistaken for a quiet
 * one.
 *
 * The rendering of one signal lives in [`SignalCard`] — shared with
 * the Overview's latest-signals block; this component owns the list,
 * the stream state and the empty state.
 *
 * [`SignalCard`]: ./signal-card.tsx
 */

import { useTranslations } from "next-intl";

import type { SignalResponse } from "@/lib/api/schema/signal";

import { SignalCard } from "./signal-card";
import { STATUS_DOT } from "./signal-display";
import { useSignalStream } from "./use-signal-stream";

export function SignalFeed({
  initial,
}: {
  initial: readonly SignalResponse[];
}) {
  const t = useTranslations("Dashboard.Signals.feed");
  const { signals, status } = useSignalStream(initial);

  return (
    <section className="mt-2 px-6 pb-10 lg:px-10">
      <div className="mb-4 flex items-center justify-between">
        <h2 className="text-[13px] font-semibold tracking-[0.28em] text-slate-400 uppercase">
          {t("title")}
        </h2>
        <span className="flex items-center gap-2 text-[13px] text-slate-400">
          <span
            className={`h-2 w-2 rounded-full ${STATUS_DOT[status]}`}
            aria-hidden
          />
          {t(`status.${status}`)}
        </span>
      </div>

      {signals.length === 0 ? (
        <p className="rounded-[8px] border border-sothoth-500/15 bg-cosmos-700/40 px-4 py-6 text-[14px] text-slate-400">
          {t("empty")}
        </p>
      ) : (
        <ul className="flex flex-col gap-2">
          {signals.map((signal) => (
            <SignalCard key={signal.id} signal={signal} />
          ))}
        </ul>
      )}
    </section>
  );
}
