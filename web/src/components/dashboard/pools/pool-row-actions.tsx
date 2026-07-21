/**
 * Per-row pool actions — the trailing "actions" cell of the pool table.
 *
 * Three utilities on one pool, right-aligned: copy the address, open it on
 * Solscan, and toggle it on the LocalStorage watchlist. Client island because
 * the copy and the star are interactive and the star reads `useWatchlist`.
 *
 * The star is *page-correlated by state*, not by page: it fills when the pool
 * is on the watchlist and empties otherwise, so on `/pools` it reads as "add",
 * on `/watchlist` (every row watched) as "remove" — one component, both jobs.
 *
 * These controls must NOT sit inside the row's navigation `<Link>` (a button
 * or link nested in an `<a>` is invalid); the row places this cell as a
 * sibling of the linked data cells.
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
