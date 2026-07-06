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
 * Local filters (severity × detector chips, `SignalFilters`) apply at
 * render time on the merged list — ephemeral UI state, the stream and
 * the 200-item merge are untouched, hidden signals reappear the
 * moment a filter releases them. Two distinct empty states: a quiet
 * feed ("the detectors are watching") vs an over-filtered one (reset
 * button).
 *
 * The rendering of one signal lives in [`SignalCard`] — shared with
 * the Overview's latest-signals block; this component owns the list,
 * the stream state, the filters and the empty states.
 *
 * [`SignalCard`]: ./signal-card.tsx
 */

import { useState } from "react";

import { useTranslations } from "next-intl";

import type { Severity, SignalResponse } from "@/lib/api/schema/signal";
import { filterSignals } from "@/lib/signals/filter-signals";

import { SignalCard } from "./signal-card";
import { STATUS_DOT } from "./signal-display";
import { SignalFilters } from "./signal-filters";
import { useSignalStream } from "./use-signal-stream";

/** Toggle one value in an immutable Set (state-update helper). */
function toggled<T>(set: ReadonlySet<T>, value: T): ReadonlySet<T> {
  const next = new Set(set);
  if (next.has(value)) {
    next.delete(value);
  } else {
    next.add(value);
  }
  return next;
}

export function SignalFeed({
  initial,
}: {
  initial: readonly SignalResponse[];
}) {
  const t = useTranslations("Dashboard.Signals.feed");
  const { signals, status } = useSignalStream(initial);

  const [activeSeverities, setActiveSeverities] = useState<
    ReadonlySet<Severity>
  >(new Set());
  const [activeDetectors, setActiveDetectors] = useState<ReadonlySet<string>>(
    new Set(),
  );

  const filtered = filterSignals(signals, activeSeverities, activeDetectors);
  const resetFilters = () => {
    setActiveSeverities(new Set());
    setActiveDetectors(new Set());
  };

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
        <>
          <SignalFilters
            signals={signals}
            activeSeverities={activeSeverities}
            activeDetectors={activeDetectors}
            onToggleSeverity={(severity) =>
              setActiveSeverities((set) => toggled(set, severity))
            }
            onToggleDetector={(detector) =>
              setActiveDetectors((set) => toggled(set, detector))
            }
          />

          {filtered.length === 0 ? (
            <div className="flex flex-wrap items-center justify-between gap-3 rounded-[8px] border border-sothoth-500/15 bg-cosmos-700/40 px-4 py-6">
              <p className="text-[14px] text-slate-400">
                {t("filters.emptyFiltered")}
              </p>
              <button
                type="button"
                onClick={resetFilters}
                className="rounded-full border border-sothoth-500/30 px-3 py-1 text-[13px] font-medium text-sothoth-200 transition-colors hover:border-sothoth-500/60 hover:bg-sothoth-600/10"
              >
                {t("filters.reset")}
              </button>
            </div>
          ) : (
            <ul className="flex flex-col gap-2">
              {filtered.map((signal) => (
                <SignalCard key={signal.id} signal={signal} />
              ))}
            </ul>
          )}
        </>
      )}
    </section>
  );
}
