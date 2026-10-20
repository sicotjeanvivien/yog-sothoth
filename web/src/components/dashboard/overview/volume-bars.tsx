"use client";

/**
 * The bars of a chart of the Overview, drawn in HTML so the server renders them at their
 * width. ⚠️ The reading sits in a fixed line above, never in a `title` or a floating tip: a
 * finger would hide it. Hover, touch, or arrow keys pick a bar; the others fade.
 */

import { useState, type KeyboardEvent } from "react";

import { LABEL_CLASS } from "./overview-layout";

export function VolumeBars({
  bars,
  ticks,
  axis,
  height,
  caption,
  invite,
  label,
}: {
  /** `height` is `null` for a slot the facts hold no whole bar for. */
  bars: readonly { height: number | null; reading: string }[];
  ticks: readonly { bottom: number; label: string }[];
  axis: readonly string[];
  height: number;
  /** A name at the left of the reading line, for a chart under another. */
  caption?: string;
  invite: string;
  label: string;
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

  return (
    <div>
      <p className="mb-2 flex min-h-[18px] items-baseline justify-between gap-2">
        {caption !== undefined && <span className={LABEL_CLASS}>{caption}</span>}
        <span aria-live="polite" className="font-dash-mono text-[12px] text-dash-ink-2">
          {active === null ? invite : bars[active]?.reading}
        </span>
      </p>
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
            className="flex h-full min-w-0 flex-1 cursor-crosshair items-end"
          >
            {bar.height !== null && (
              <div
                className={`w-full rounded-t-[2px] bg-dash-accent transition-opacity ${
                  active !== null && active !== i ? "opacity-35" : ""
                }`}
                style={{ height: bar.height }}
              />
            )}
          </div>
        ))}
      </div>
      <div className="mt-1.5 flex justify-between font-dash-mono text-[11px] text-dash-ink-3">
        {axis.map((tick, i) => (
          <span key={i}>{tick}</span>
        ))}
      </div>
    </div>
  );
}
