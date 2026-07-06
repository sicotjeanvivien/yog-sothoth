"use client";

/**
 * Info popover — an ⓘ button that reveals contextual help on demand.
 *
 * The house pattern for "useful to whoever wonders, invisible to
 * everyone else": page descriptions, KPI definitions, detector
 * explanations… anywhere the dashboard needs to explain itself
 * without paying permanent screen height for it.
 *
 * Deliberately a *click-toggled popover*, not a hover tooltip: hover
 * doesn't exist on touch, so a hover-only bubble would make the help
 * unreachable on mobile. Closes on outside pointer-down or Escape.
 * Accessible: a real `<button>` carrying `aria-label` (the icon has
 * no text), `aria-expanded` and `aria-controls`.
 *
 * The panel resets its own typography — it may sit next to a display
 * heading and must not inherit its size/weight/tracking.
 */

import { useEffect, useId, useRef, useState, type ReactNode } from "react";

import { InfoIcon } from "./icon";

export function InfoPopover({
  label,
  children,
  iconSize = 16,
}: {
  /** Accessible name of the ⓘ button (it renders no text). */
  label: string;
  /** Panel content. */
  children: ReactNode;
  iconSize?: number;
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLSpanElement>(null);
  const panelId = useId();

  // Global listeners only while open: outside pointer-down or Escape
  // closes. `pointerdown` (not click) so the panel is gone before any
  // other interaction the press initiates.
  useEffect(() => {
    if (!open) {
      return;
    }
    const onPointerDown = (event: PointerEvent) => {
      if (
        rootRef.current &&
        !rootRef.current.contains(event.target as Node)
      ) {
        setOpen(false);
      }
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
      }
    };
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

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
