"use client";

/**
 * Bars stacked by part, each part in its tone. Under the chart, the period's detail, replaced by
 * the bar's under hover, touch or arrow keys.
 */

import { useState, type KeyboardEvent } from "react";

import { LABEL_CLASS } from "@/components/dashboard/overview/overview-layout";

export type SplitDetail<T extends string> = {
  when: string;
  total: string;
  rows: readonly { tone: T; name: string; value: string; share: string }[];
};

export function SplitBars<T extends string>({
  bars,
  period,
  ticks,
  axis,
  height,
  label,
  tones,
}: {
  /** Heights in pixels, bottom first; no segment for a bar without a value. */
  bars: readonly { segments: readonly { tone: T; height: number }[]; detail: SplitDetail<T> }[];
  period: SplitDetail<T>;
  ticks: readonly { bottom: number; label: string }[];
  axis: readonly string[];
  height: number;
  label: string;
  /** The class of each tone, shared with the legend the server draws. */
  tones: Record<T, string>;
}) {
  const [active, setActive] = useState<number | null>(null);

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const step = event.key === "ArrowRight" ? 1 : event.key === "ArrowLeft" ? -1 : 0;
    if (step === 0 || bars.length === 0) return;
    event.preventDefault();
    setActive((current) =>
      current === null
        ? step > 0
          ? 0
          : bars.length - 1
        : Math.min(bars.length - 1, Math.max(0, current + step)),
    );
  };
  const detail = (active === null ? undefined : bars[active]?.detail) ?? period;

  return (
    <div>
      <div
        role="img"
        aria-label={label}
        tabIndex={0}
        onKeyDown={onKeyDown}
        onMouseLeave={() => setActive(null)}
        onBlur={() => setActive(null)}
        className="relative flex items-end gap-0.5 border-b border-dash-rule"
        style={{ height }}
      >
        {ticks.map((tick) => (
          <span
            key={tick.label}
            aria-hidden="true"
            className="pointer-events-none absolute inset-x-0 z-[1] h-0 border-t border-dashed border-dash-ink-3/30"
            style={{ bottom: tick.bottom }}
          >
            <span className="absolute bottom-0.5 left-0 bg-dash-surface pr-1 font-dash-mono text-[11px] leading-none text-dash-ink-3">
              {tick.label}
            </span>
          </span>
        ))}
        {bars.map((bar, i) => (
          <div
            key={i}
            onMouseEnter={() => setActive(i)}
            onClick={() => setActive(i)}
            className={`flex h-full min-w-0 flex-1 cursor-crosshair flex-col-reverse gap-0.5 transition-opacity ${
              active !== null && active !== i ? "opacity-35" : ""
            }`}
          >
            {bar.segments.map((segment, j) => (
              <div
                key={segment.tone}
                className={`w-full shrink-0 ${tones[segment.tone]} ${
                  j === bar.segments.length - 1 ? "rounded-t-[2px]" : ""
                }`}
                style={{ height: segment.height }}
              />
            ))}
          </div>
        ))}
      </div>
      <div className="mt-1.5 flex justify-between font-dash-mono text-[11px] text-dash-ink-3">
        {axis.map((tick, i) => (
          <span key={i}>{tick}</span>
        ))}
      </div>
      <div aria-live="polite" className="mt-3 border-t border-dash-rule pt-2.5">
        <div className="mb-1 flex items-baseline justify-between gap-3">
          <span className={LABEL_CLASS}>{detail.when}</span>
          <span className="font-dash-mono text-[14px] text-dash-ink tabular-nums">{detail.total}</span>
        </div>
        {detail.rows.map((row) => (
          <div
            key={row.tone}
            className="grid grid-cols-[minmax(0,1fr)_auto_56px] items-baseline gap-3 py-1 text-[14px] text-dash-ink-2"
          >
            <span>
              <span
                aria-hidden="true"
                className={`mr-1.5 inline-block h-2.5 w-2.5 align-[-1px] ${tones[row.tone]}`}
              />
              {row.name}
            </span>
            <span className="text-right font-dash-mono tabular-nums">{row.value}</span>
            <span className="text-right font-dash-mono text-dash-ink-3 tabular-nums">{row.share}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
