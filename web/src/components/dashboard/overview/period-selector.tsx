"use client";

/**
 * 24H / 7D / 30D, links that change `?period=` and keep the page's other parameters. A closed
 * period is a button that opens why, not `aria-disabled`. ⚠️ No `title`, never shown on touch.
 * Without `availability` (still loading), the other periods wait, inert.
 */

import { useCallback, useId, useRef, useState } from "react";

import { useDismiss } from "@/components/shared/use-dismiss";
import type { PagePath } from "@/config/site";
import { Link } from "@/i18n/navigation";
import { pageQuery, PERIODS, type Period } from "@/lib/overview/period";

const SEGMENT_CLASS =
  "inline-flex min-h-9 min-w-11 items-center justify-center border-r border-dash-rule px-3 font-dash-mono text-[12px] last:border-r-0";

/** Per period, open, or closed with the reason its panel shows. */
export type PeriodAvailability = Record<Period, { open: true } | { open: false; reason: string }>;

export function PeriodSelector({
  pathname,
  keep,
  current,
  availability,
  labels,
  closedLabels,
  groupLabel,
}: {
  pathname: PagePath;
  /** The page's other query parameters, kept across a period change. */
  keep: Record<string, string>;
  current: Period;
  availability: PeriodAvailability | null;
  labels: Record<Period, string>;
  closedLabels: Record<Period, string>;
  groupLabel: string;
}) {
  return (
    <div role="group" aria-label={groupLabel} className="inline-flex border border-dash-rule">
      {PERIODS.map((period) => {
        if (period === current) {
          return (
            <span
              key={period}
              aria-current="page"
              className={`${SEGMENT_CLASS} bg-dash-accent font-semibold text-dash-surface`}
            >
              {labels[period]}
            </span>
          );
        }
        const state = availability?.[period];
        if (state === undefined) {
          return (
            <span key={period} className={`${SEGMENT_CLASS} text-dash-ink-3`}>
              {labels[period]}
            </span>
          );
        }
        if (!state.open) {
          return (
            <ClosedPeriod
              key={period}
              label={labels[period]}
              closedLabel={closedLabels[period]}
              reason={state.reason}
            />
          );
        }
        return (
          <Link
            key={period}
            href={{ pathname, query: pageQuery(period, keep) }}
            scroll={false}
            className={`${SEGMENT_CLASS} text-dash-ink-2 hover:bg-dash-surface hover:text-dash-ink`}
          >
            {labels[period]}
          </Link>
        );
      })}
    </div>
  );
}

function ClosedPeriod({
  label,
  closedLabel,
  reason,
}: {
  label: string;
  closedLabel: string;
  reason: string;
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLSpanElement>(null);
  const panelId = useId();
  const close = useCallback(() => setOpen(false), []);
  useDismiss(rootRef, open, close);

  return (
    <span ref={rootRef} className="relative inline-flex border-r border-dash-rule last:border-r-0">
      <button
        type="button"
        aria-label={closedLabel}
        aria-expanded={open}
        aria-controls={panelId}
        onClick={() => setOpen((value) => !value)}
        className={`${SEGMENT_CLASS} cursor-help border-r-0 text-dash-ink-3/60`}
      >
        {label}
      </button>
      {/* Rendered closed too, `hidden`: `aria-controls` must point at an element. */}
      <div
        id={panelId}
        role="note"
        hidden={!open}
        className="absolute top-full right-0 z-20 mt-1 w-[260px] max-w-[70vw] border border-dash-rule bg-dash-surface-2 px-3 py-2.5 text-left text-[12px] leading-[1.5] text-dash-ink-2"
      >
        {reason}
      </div>
    </span>
  );
}
