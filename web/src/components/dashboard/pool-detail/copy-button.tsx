"use client";

/**
 * Copy-to-clipboard button.
 *
 * Isolated Client Component so the rest of the page stays on the
 * server. Renders a small icon-only button next to whatever text
 * the caller wants to copy; on click, writes `value` to the
 * clipboard and shows a brief checkmark confirmation.
 *
 * Two design choices worth flagging:
 *
 *   - Confirmation lives in local state, not in the URL or in any
 *     global store. The button is self-contained.
 *
 *   - `aria-label` is required (the button has no visible text),
 *     so the caller passes a localised label — copying is a generic
 *     UI affordance, the surrounding context disambiguates which
 *     value is being copied.
 */

import { useState } from "react";

import { CopyIcon, CheckIcon } from "@/components/shared/icon";

const BUTTON_CLASS =
  "inline-flex h-6 w-6 items-center justify-center rounded-[3px] text-slate-400 transition-colors hover:bg-sothoth-500/15 hover:text-sothoth-300";

export function CopyButton({
  value,
  label,
}: {
  value: string;
  label: string;
}) {
  const [copied, setCopied] = useState(false);

  const handleClick = async () => {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard API can fail in insecure contexts or when the
      // user denies permission. We silently no-op rather than
      // showing an error — the worst case is the user sees no
      // confirmation and tries again.
    }
  };

  return (
    <button
      type="button"
      onClick={handleClick}
      className={BUTTON_CLASS}
      aria-label={label}
    >
      {copied ? <CheckIcon size={14} /> : <CopyIcon size={14} />}
    </button>
  );
}