"use client";

import { useRouter } from "next/navigation";
import { useTransition } from "react";

type RetryButtonProps = {
  label: string;
  pendingLabel: string;
  variant?: "block" | "page";
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
    "inline-flex items-center justify-center rounded-md font-medium transition-colors " +
    "disabled:cursor-not-allowed disabled:opacity-60 " +
    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-violet-400 focus-visible:ring-offset-2 focus-visible:ring-offset-slate-950";

  const variantClasses =
    variant === "page"
      ? "px-4 py-2 text-sm bg-violet-500 text-white hover:bg-violet-400"
      : "px-4 py-2 text-[17px] border-amber-500/40 bg-amber-500/15 text-amber-400 hover:bg-amber-700 border border-amber-700";

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