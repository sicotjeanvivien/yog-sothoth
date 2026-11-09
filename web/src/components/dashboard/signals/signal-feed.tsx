"use client";

/**
 * The live signal feed, seeded by the server page and kept live by `useLiveSignals`; the
 * status dot keeps a broken stream from passing for a quiet one, a gap row marks a reopening.
 * Filters apply at render time, leaving the stream and the merge untouched.
 */

import { useState } from "react";

import { useLocale, useTranslations } from "next-intl";

import type { SeverityResponse, SignalResponse } from "@/lib/api/schema/zod.gen";
import { formatUtcTime } from "@/lib/format/date";
import { filterSignals } from "@/lib/signals/filter-signals";

import { SignalCard } from "./signal-card";
import { STATUS_DOT } from "./signal-display";
import { SignalFilters } from "./signal-filters";
import { useLiveSignals } from "./use-live-signals";

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
  const { rows, status } = useLiveSignals(initial);
  const signals = rows.flatMap((row) => (row.type === "signal" ? [row.signal] : []));

  const [activeSeverities, setActiveSeverities] = useState<
    ReadonlySet<SeverityResponse>
  >(new Set());
  const [activeDetectors, setActiveDetectors] = useState<ReadonlySet<string>>(
    new Set(),
  );

  const filtered = filterSignals(rows, activeSeverities, activeDetectors);
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

          {!filtered.some((row) => row.type === "signal") ? (
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
              {filtered.map((row) =>
                row.type === "gap" ? (
                  <SignalGapItem key={row.key} at={row.at} />
                ) : (
                  <SignalCard key={row.key} signal={row.signal} />
                ),
              )}
            </ul>
          )}
        </>
      )}
    </section>
  );
}

function SignalGapItem({ at }: { at: string }) {
  const t = useTranslations("Dashboard.Signals.feed");
  const locale = useLocale();
  return (
    <li className="px-4 py-1.5 font-dash-mono text-[12px] text-slate-500">
      {t("gap", { at: formatUtcTime(at, locale) })}
    </li>
  );
}
