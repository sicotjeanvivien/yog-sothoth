"use client";

/** "Advanced mode", a switch beside the period selector. */

import { useOverviewMode } from "./overview-mode";

export function AdvancedSwitch({ label }: { label: string }) {
  const { advanced, toggle } = useOverviewMode();

  return (
    <button
      type="button"
      role="switch"
      aria-checked={advanced}
      onClick={toggle}
      className="flex min-h-9 items-center gap-3 border border-dash-rule px-3 text-[12px] text-dash-ink-2 hover:border-dash-ink-3 hover:text-dash-ink"
    >
      <span>{label}</span>
      <span
        aria-hidden="true"
        className={`relative h-[18px] w-[34px] shrink-0 border ${
          advanced ? "border-dash-accent bg-dash-accent" : "border-dash-ink-3 bg-dash-surface-2"
        }`}
      >
        <span
          className={`absolute top-[3px] h-2.5 w-2.5 ${
            advanced ? "left-[19px] bg-dash-surface" : "left-[3px] bg-dash-ink-3"
          }`}
        />
      </span>
    </button>
  );
}
