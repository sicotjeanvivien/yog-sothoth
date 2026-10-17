"use client";

/** The wallet's "Copy" button, a client island: its label turns to "Copied" for a moment. */

import { useState } from "react";

import { SECONDARY_BUTTON_CLASS } from "@/components/shared/secondary-button";

export function CopyAddressButton({
  value,
  label,
  copiedLabel,
  describedBy,
}: {
  value: string;
  label: string;
  copiedLabel: string;
  /** The id of the element showing `value`. */
  describedBy: string;
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
      aria-describedby={describedBy}
      className={SECONDARY_BUTTON_CLASS}
    >
      {copied ? copiedLabel : label}
    </button>
  );
}
