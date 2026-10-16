/**
 * A pool row's actions: copy, Solscan, watchlist star — filled when watched, so "add" on
 * `/pools` and "remove" on `/watchlist`.
 */

"use client";

import { useTranslations } from "next-intl";

import { CopyButton } from "@/components/shared/copy-button";
import { SolscanIcon, WatchlistIcon } from "@/components/shared/icon";
import { useWatchlist } from "@/lib/watchlist/use-watchlist";

const ICON_BUTTON =
  "inline-flex h-6 w-6 items-center justify-center rounded-[3px] text-slate-400 transition-colors hover:bg-sothoth-500/15 hover:text-sothoth-300";

export function PoolRowActions({ address }: { address: string }) {
  const t = useTranslations("Dashboard.Pools.rowActions");
  const tWatch = useTranslations("Dashboard.Watchlist.toggle");
  const { has, toggle } = useWatchlist();
  const watched = has(address);

  return (
    <div className="flex items-center justify-end gap-1">
      <CopyButton value={address} label={t("copyAddress")} />

      <a
        href={`https://solscan.io/account/${address}`}
        target="_blank"
        rel="noopener noreferrer"
        aria-label={t("viewOnSolscan")}
        className={ICON_BUTTON}
      >
        <SolscanIcon size={14} />
      </a>

      <button
        type="button"
        onClick={() => toggle(address)}
        aria-pressed={watched}
        aria-label={watched ? tWatch("remove") : tWatch("add")}
        className={`${ICON_BUTTON} ${watched ? "text-sothoth-300 hover:text-sothoth-100" : ""}`}
      >
        <WatchlistIcon size={14} className={watched ? "fill-current" : ""} />
      </button>
    </div>
  );
}
