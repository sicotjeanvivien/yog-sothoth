"use client";

/**
 * Liquidity added above the axis and removed under it, one column per bar, with symmetric
 * marks. ⚠️ As `VolumeBars`: the reading in a fixed line above, picked by hover, touch or arrow
 * keys, the other columns faded.
 */

import { useState, type KeyboardEvent } from "react";

export function FlowBars({
  bars,
  ticks,
  axis,
  half,
  invite,
  label,
}: {
  /** Heights in pixels; `null` for a bar the liquidity was not measured whole. */
  bars: readonly { up: number | null; down: number | null; reading: string }[];
  /** Each mark's distance from the axis, and its unsigned label. */
  ticks: readonly { offset: number; label: string }[];
  axis: readonly string[];
  half: number;
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
  const faded = (i: number) => (active !== null && active !== i ? "opacity-35" : "");

  return (
    <div>
      <p
        aria-live="polite"
        className="mb-2 min-h-[18px] font-dash-mono text-[12px] text-dash-ink-2"
      >
        {active === null ? invite : bars[active]?.reading}
      </p>
      <div
        role="img"
        aria-label={label}
        tabIndex={0}
        onKeyDown={onKeyDown}
        onMouseLeave={() => setActive(null)}
        onBlur={() => setActive(null)}
        className="relative"
        style={{ height: 2 * half + 1 }}
      >
        {ticks.flatMap((tick) =>
          [
            { top: half - tick.offset, text: `+${tick.label}` },
            { top: half + 1 + tick.offset, text: `−${tick.label}` },
          ].map((mark) => (
            <span
              key={mark.text}
              aria-hidden="true"
              className="pointer-events-none absolute inset-x-0 z-[1] h-0 border-t border-dashed border-dash-ink-3/30"
              style={{ top: mark.top }}
            >
              <span className="absolute bottom-0.5 left-0 bg-dash-surface pr-1 font-dash-mono text-[11px] leading-none text-dash-ink-3">
                {mark.text}
              </span>
            </span>
          )),
        )}
        <span
          aria-hidden="true"
          className="absolute inset-x-0 h-px bg-dash-rule"
          style={{ top: half }}
        />
        <div className="absolute inset-0 flex gap-0.5">
          {bars.map((bar, i) => (
            <div
              key={i}
              onMouseEnter={() => setActive(i)}
              onClick={() => setActive(i)}
              className="flex h-full min-w-0 flex-1 cursor-crosshair flex-col"
            >
              <div className="flex flex-1 items-end" style={{ height: half }}>
                {bar.up !== null && (
                  <div
                    className={`w-full rounded-t-[2px] bg-dash-up transition-opacity ${faded(i)}`}
                    style={{ height: bar.up }}
                  />
                )}
              </div>
              <div className="h-px" />
              <div className="flex-1" style={{ height: half }}>
                {bar.down !== null && (
                  <div
                    className={`w-full rounded-b-[2px] bg-dash-down transition-opacity ${faded(i)}`}
                    style={{ height: bar.down }}
                  />
                )}
              </div>
            </div>
          ))}
        </div>
      </div>
      <div className="mt-1.5 flex justify-between font-dash-mono text-[11px] text-dash-ink-3">
        {axis.map((tick, i) => (
          <span key={i}>{tick}</span>
        ))}
      </div>
    </div>
  );
}
