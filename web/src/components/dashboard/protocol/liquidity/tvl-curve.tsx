"use client";

/**
 * The TVL curve on a tight axis, drawn by the server's coordinates. ⚠️ The reading sits in a
 * fixed line above, never in a floating tip: a finger would hide it. Hover, touch, or arrow keys
 * pick a point and drop a dashed mark on it.
 */

import { useState, type KeyboardEvent } from "react";

import { CURVE_BOX } from "@/lib/protocol/liquidity";

export function TvlCurve({
  line,
  area,
  marks,
  readings,
  rest,
  low,
  high,
  last,
  label,
}: {
  line: string;
  area: string;
  /** Each point's place along the width, in percent. */
  marks: readonly number[];
  readings: readonly string[];
  /** The reading while no point is picked. */
  rest: string;
  low: string;
  high: string;
  last: string;
  label: string;
}) {
  const [active, setActive] = useState<number | null>(null);

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const step = event.key === "ArrowRight" ? 1 : event.key === "ArrowLeft" ? -1 : 0;
    if (step === 0 || marks.length === 0) return;
    event.preventDefault();
    setActive((current) =>
      current === null
        ? step > 0
          ? 0
          : marks.length - 1
        : Math.min(marks.length - 1, Math.max(0, current + step)),
    );
  };

  return (
    <div>
      <p
        aria-live="polite"
        className="mb-2 min-h-[18px] font-dash-mono text-[12px] text-dash-ink-2"
      >
        {active === null ? rest : readings[active]}
      </p>
      <div
        role="img"
        aria-label={label}
        tabIndex={0}
        onKeyDown={onKeyDown}
        onMouseLeave={() => setActive(null)}
        onBlur={() => setActive(null)}
        className="relative h-[140px] border-b border-dash-rule"
      >
        {active !== null && (
          <span
            aria-hidden="true"
            className="pointer-events-none absolute inset-y-0 z-[1] w-0 border-l border-dashed border-dash-ink-3"
            style={{ left: `${marks[active]}%` }}
          />
        )}
        <div className="absolute inset-0 z-[2] flex">
          {marks.map((_, i) => (
            <div
              key={i}
              onMouseEnter={() => setActive(i)}
              onClick={() => setActive(i)}
              className="h-full flex-1 cursor-crosshair"
            />
          ))}
        </div>
        <svg
          viewBox={`0 0 ${CURVE_BOX.width} ${CURVE_BOX.height}`}
          preserveAspectRatio="none"
          aria-hidden="true"
          className="absolute inset-0 h-full w-full"
        >
          <path d={area} className="fill-dash-accent/10" />
          <path
            d={line}
            fill="none"
            className="stroke-dash-accent"
            strokeWidth={2}
            vectorEffect="non-scaling-stroke"
          />
        </svg>
        <span className="absolute top-0 left-0 bg-dash-surface pr-1 font-dash-mono text-[11px] leading-none text-dash-ink-3">
          {high}
        </span>
        <span className="absolute bottom-1 left-0 bg-dash-surface pr-1 font-dash-mono text-[11px] leading-none text-dash-ink-3">
          {low}
        </span>
        <span className="absolute top-0 right-0 bg-dash-surface pl-1 font-dash-mono text-[12px] leading-none text-dash-ink">
          {last}
        </span>
      </div>
    </div>
  );
}
