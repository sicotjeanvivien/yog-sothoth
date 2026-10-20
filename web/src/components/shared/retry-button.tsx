"use client";

import { useRouter } from "next/navigation";
import { useTransition } from "react";

type RetryButtonProps = {
  label: string;
  pendingLabel: string;
  /** `box` and `link`: the Overview's error box and KPI, in the dashboard's colours. */
  variant?: "block" | "page" | "box" | "link";
};

export function RetryButton({ label, pendingLabel, variant = "block" }: RetryButtonProps) {
  const router = useRouter();
  const [isPending, startTransition] = useTransition();

  const handleClick = () => {
    startTransition(() => {
      router.refresh();
    });
  };

  const baseClasses =
    "inline-flex items-center justify-center font-medium transition-colors " +
    "disabled:cursor-not-allowed disabled:opacity-60 " +
    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-violet-400 focus-visible:ring-offset-2 focus-visible:ring-offset-slate-950";

  const variantClasses = {
    page: "rounded-md px-4 py-2 text-sm bg-violet-500 text-white hover:bg-violet-400",
    block:
      "rounded-md px-4 py-2 text-[17px] border-amber-500/40 bg-amber-500/15 text-amber-400 hover:bg-amber-700 border border-amber-700",
    box: "min-h-9 px-3 text-[14px] border border-dash-rule text-dash-ink hover:border-dash-ink-3",
    link: "min-h-7 self-start text-[12px] text-dash-accent-2 hover:underline",
  }[variant];

  return (
    <button
      type="button"
      onClick={handleClick}
      disabled={isPending}
      className={`${baseClasses} ${variantClasses}`}
      aria-busy={isPending}
    >
      {isPending ? pendingLabel : label}
    </button>
  );
}