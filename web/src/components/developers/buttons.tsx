"use client";

import { useEffect, useRef, useState, type ButtonHTMLAttributes } from "react";

const GHOST =
  "border border-dash-rule bg-transparent text-[12px] text-dash-ink transition-colors hover:border-dash-ink-3 disabled:cursor-not-allowed disabled:text-dash-ink-3 disabled:hover:border-dash-rule";

export function GhostButton({
  small = false,
  className = "",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { small?: boolean }) {
  return (
    <button
      type="button"
      {...props}
      className={`${GHOST} ${small ? "min-h-7 px-2.5" : "min-h-9 px-3.5"} ${className}`}
    />
  );
}

export function PrimaryButton(props: ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      type="button"
      {...props}
      className="min-h-9 border border-dash-accent bg-dash-accent px-[18px] text-[14px] font-semibold text-dash-surface transition-colors hover:bg-dash-accent-2 disabled:cursor-not-allowed disabled:border-dash-rule disabled:bg-transparent disabled:text-dash-ink-3"
    />
  );
}

/** Copies `text`, then reads "Copied" for a moment. */
export function CopyButton({
  text,
  label,
  copied,
  small = false,
}: {
  text: string;
  label: string;
  copied: string;
  small?: boolean;
}) {
  const [done, setDone] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => () => clearTimeout(timer.current), []);

  const copy = () => {
    navigator.clipboard?.writeText(text).then(
      () => {
        setDone(true);
        clearTimeout(timer.current);
        timer.current = setTimeout(() => setDone(false), 1400);
      },
      () => {},
    );
  };

  return (
    <>
      <GhostButton small={small} onClick={copy}>
        {done ? copied : label}
      </GhostButton>
      {/* ⚠️ A focused button's new label is rarely read out: the live region announces it. */}
      <span aria-live="polite" className="sr-only">
        {done ? copied : ""}
      </span>
    </>
  );
}
