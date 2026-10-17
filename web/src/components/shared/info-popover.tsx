"use client";

/**
 * An ⓘ button revealing help on demand. ⚠️ Click-toggled, not hover: hover does not exist on
 * touch. The panel resets its typography, as it may sit next to a display heading.
 */

import { useCallback, useId, useRef, useState, type ReactNode } from "react";

import { InfoIcon } from "./icon";
import { useDismiss } from "./use-dismiss";

export function InfoPopover({
  label,
  children,
  iconSize = 16,
}: {
  /** Accessible name of the ⓘ button (it renders no text). */
  label: string;
  children: ReactNode;
  iconSize?: number;
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLSpanElement>(null);
  const panelId = useId();

  const close = useCallback(() => setOpen(false), []);
  useDismiss(rootRef, open, close);

  return (
    <span ref={rootRef} className="relative inline-flex">
      <button
        type="button"
        aria-label={label}
        aria-expanded={open}
        aria-controls={panelId}
        onClick={() => setOpen((value) => !value)}
        className={`inline-flex items-center transition-colors ${
          open ? "text-slate-200" : "text-slate-500 hover:text-slate-300"
        }`}
      >
        <InfoIcon size={iconSize} />
      </button>

      {open && (
        <div
          id={panelId}
          role="note"
          className="absolute top-full left-0 z-20 mt-2 w-max max-w-[42ch] rounded-[8px] border border-sothoth-500/25 bg-cosmos-800 px-4 py-3 font-sans text-[13px] leading-[1.55] font-normal tracking-normal text-slate-300 normal-case shadow-[0_8px_24px_rgba(0,0,0,0.45)]"
        >
          {children}
        </div>
      )}
    </span>
  );
}
