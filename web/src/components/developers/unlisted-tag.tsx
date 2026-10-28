"use client";

import { useCallback, useId, useRef, useState } from "react";
import { useTranslations } from "next-intl";

import { useDismiss } from "@/components/shared/use-dismiss";

/** "Unlisted", and on a press why: no link leads to this page. */
export function UnlistedTag() {
  const t = useTranslations("Developers.unlisted");
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLSpanElement>(null);
  const panelId = useId();
  const close = useCallback(() => setOpen(false), []);
  useDismiss(rootRef, open, close);

  return (
    <span ref={rootRef} className="relative inline-flex self-center">
      <button
        type="button"
        aria-expanded={open}
        aria-controls={panelId}
        onClick={() => setOpen((value) => !value)}
        className="ml-2 border border-dashed border-dash-ink-3 px-[7px] py-px font-dash-mono text-[11px] tracking-[0.08em] text-dash-ink-3 uppercase hover:border-dash-ink-2 hover:text-dash-ink aria-expanded:border-dash-ink-2 aria-expanded:text-dash-ink"
      >
        {t("label")}
      </button>
      {open && (
        <span
          id={panelId}
          role="note"
          className="absolute top-[calc(100%+6px)] left-0 z-20 w-[260px] max-w-[70vw] border border-dash-rule bg-dash-surface-2 px-3 py-2.5 text-[12px] leading-normal text-dash-ink-2"
        >
          {t("body")}
        </span>
      )}
    </span>
  );
}
