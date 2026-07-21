/**
 * Watchlist star toggle.
 *
 * Client island: adds/removes a pool from the LocalStorage watchlist via
 * `useWatchlist`. The star is `WatchlistIcon` — outline when off, filled
 * (`fill-current`) when the pool is watched. `aria-pressed` exposes the toggle
 * state; the label flips between "Watch" and "Watching".
 *
 * SSR-safe by construction: `useWatchlist` renders "empty" on the server and
 * the first client paint, so this button starts as "Watch" and flips to
 * "Watching" after hydration if the pool is already on the list — no mismatch.
 */

"use client";

import { useTranslations } from "next-intl";

import { WatchlistIcon } from "@/components/shared/icon";
import { useWatchlist } from "@/lib/watchlist/use-watchlist";

export function WatchlistToggle({ address }: { address: string }) {
  const t = useTranslations("Dashboard.Watchlist.toggle");
  const { has, toggle } = useWatchlist();
  const active = has(address);

  return (
    <button
      type="button"
      onClick={() => toggle(address)}
      aria-pressed={active}
      aria-label={active ? t("remove") : t("add")}
      className={`inline-flex items-center justify-center gap-2 rounded-[4px] border px-4 py-[8px] text-[14px] font-semibold transition-colors ${
        active
          ? "border-sothoth-400/50 bg-sothoth-500/10 text-sothoth-100 hover:border-sothoth-400/70"
          : "border-slate-700 bg-transparent text-slate-200 hover:border-slate-500 hover:bg-slate-800/40"
      }`}
    >
      <WatchlistIcon
        size={14}
        className={active ? "fill-current text-sothoth-300" : ""}
      />
      {active ? t("watching") : t("watch")}
    </button>
  );
}
