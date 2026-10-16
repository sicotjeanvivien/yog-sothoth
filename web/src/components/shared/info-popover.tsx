"use client";

/**
 * An ⓘ button revealing help on demand. ⚠️ Click-toggled, not hover: hover does not exist on
 * touch. The panel resets its typography, as it may sit next to a display heading.
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
  children: ReactNode;
  iconSize?: number;
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLSpanElement>(null);
  const panelId = useId();

  // `pointerdown`, not click: the panel is gone before what the press starts.
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
