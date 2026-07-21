/**
 * Pool detail page — header block.
 *
 * Structure (mobile-first, two-column on desktop):
 *
 *   ┌───────────────────────────────────────────────────┐
 *   │  ← Back to Pools                                  │
 *   │                                                   │
 *   │  [logos] SOL / USDC        [★ Watch] [Solscan]    │
 *   │  [icon] DAMM v2                                   │
 *   └───────────────────────────────────────────────────┘
 *
 * The back link and the right-hand actions (watchlist toggle + external
 * CTAs) land on the same row as the identity block on desktop, stack
 * vertically on mobile. The pool address + copy now live in the Pool info
 * block below (and in each pool row's actions), so the header stays lean.
 *
 * The pair symbols and the protocol badge flow from already-validated
 * server data — no conditional fallback here; if a token has no symbol the
 * empty string flows through and the layout simply shows nothing for that
 * side. That happens rarely enough not to warrant special UI; the Pool info
 * block below surfaces the mints verbatim if the visitor cares.
 */

import { getTranslations } from "next-intl/server";

import { Link } from "@/i18n/navigation";

import { ArrowLeftIcon, ExternalLinkIcon } from "@/components/shared/icon";

import type { PoolResponse } from "@/lib/api/schema/pool";
import { formatMeteoraUrl } from "@/lib/format/format-meteora-url";

import { PoolPairCell } from "@/components/dashboard/pools/pool-pair-cell";
import { ProtocolBadge } from "@/components/dashboard/pools/protocol-badge";
import { WatchlistToggle } from "@/components/dashboard/watchlist/watchlist-toggle";

const CTA_CLASS =
  "inline-flex items-center justify-center gap-2 rounded-[4px] border border-slate-700 bg-transparent px-4 py-[8px] text-[14px] font-semibold text-slate-200 transition-colors hover:border-slate-500 hover:bg-slate-800/40";

export async function PoolDetailHeader({ pool }: { pool: PoolResponse }) {
  const t = await getTranslations("Dashboard.PoolDetail.header");

  const meteoraUrl = formatMeteoraUrl(pool.protocol, pool.poolAddress);
  const solscanUrl = `https://solscan.io/account/${pool.poolAddress}`;

  return (
    <header className="px-6 pt-6 pb-4 lg:px-10">
      {/* Back link */}
      <Link
        href="/pools"
        className="inline-flex items-center gap-1.5 text-[14px] text-slate-400 transition-colors hover:text-slate-200"
      >
        <ArrowLeftIcon size={14} />
        {t("backToList")}
      </Link>

      {/* Identity + CTAs */}
      <div className="mt-6 grid grid-cols-1 items-start gap-6 lg:grid-cols-[1fr_auto]">
        {/* Left — pair identity */}
        <div>
          {/* Token pair */}
          <div className="flex items-center gap-3">
            <PoolPairCell tokenA={pool.tokenA} tokenB={pool.tokenB} />
          </div>
          {/* Protocol badge on its own line below the pair */}
          <div className="mt-3 flex items-center gap-2 text-[14px] text-slate-400">
            <ProtocolBadge protocol={pool.protocol} />
          </div>
        </div>

        {/* Right — watchlist toggle + external CTAs */}
        <div className="flex flex-wrap gap-2">
          <WatchlistToggle address={pool.poolAddress} />
          {meteoraUrl && (
            <a
              href={meteoraUrl}
              target="_blank"
              rel="noopener noreferrer"
              className={CTA_CLASS}
            >
              {t("viewOnMeteora")}
              <ExternalLinkIcon size={12} />
            </a>
          )}
          <a
            href={solscanUrl}
            target="_blank"
            rel="noopener noreferrer"
            className={CTA_CLASS}
          >
            {t("viewOnSolscan")}
            <ExternalLinkIcon size={12} />
          </a>
        </div>
      </div>
    </header>
  );
}