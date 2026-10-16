"use client";

/**
 * An icon-only copy button, a client island; `aria-label` is required, the button showing no
 * text.
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
      // Insecure context or denied permission: no confirmation, no error.
    }
  };

  return (
    <button
      type="button"
      onClick={handleClick}
      className={BUTTON_CLASS}
      aria-label={label}
    >
      {copied ? <CheckIcon size={16} /> : <CopyIcon size={16} />}
    </button>
  );
}