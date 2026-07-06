/**
 * Pool detail page — header block.
 *
 * Structure (mobile-first, two-column on desktop):
 *
 *   ┌───────────────────────────────────────────────────┐
 *   │  ← Back to Pools                                  │
 *   │                                                   │
 *   │  [logos] SOL / USDC          [Meteora] [Solscan]  │
 *   │          Meteora DAMM v2                          │
 *   │          7xKX...23Ab  [copy]                      │
 *   └───────────────────────────────────────────────────┘
 *
 * The back link and the two external CTAs land on the same row as
 * the identity block on desktop, stack vertically on mobile.
 *
 * All identifying labels (symbols, protocol display name, short
 * address) flow from already-validated server data — there is no
 * conditional fallback in this component; if a token has no symbol
 * the empty string flows through and the layout simply shows
 * nothing for that side. That happens rarely enough not to warrant
 * special UI; the Pool info block below will surface the mints
 * verbatim if the visitor cares.
 */

import { getTranslations } from "next-intl/server";

import { Link } from "@/i18n/navigation";

import { ArrowLeftIcon, ExternalLinkIcon } from "@/components/shared/icon";

import type { PoolResponse } from "@/lib/api/schema/pool";
import { formatMeteoraUrl } from "@/lib/format/format-meteora-url";
import { formatProtocolLabel } from "@/lib/format/format-protocol";
import { formatShortAddress } from "@/lib/format/format-short-address";

import { PoolPairCell } from "@/components/dashboard/pools/pool-pair-cell";
import { CopyButton } from "@/components/shared/copy-button";

const CTA_CLASS =
  "inline-flex items-center justify-center gap-2 rounded-[4px] border border-slate-700 bg-transparent px-4 py-[8px] text-[14px] font-semibold text-slate-200 transition-colors hover:border-slate-500 hover:bg-slate-800/40";

export async function PoolDetailHeader({ pool }: { pool: PoolResponse }) {
  const t = await getTranslations("Dashboard.PoolDetail.header");

  const meteoraUrl = formatMeteoraUrl(pool.protocol, pool.poolAddress);
  const solscanUrl = `https://solscan.io/account/${pool.poolAddress}`;

  return (
    <header className="px-6 pt-8 pb-6 lg:px-10 lg:pt-10">
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
          {/* Logos + pair + protocol on a single line, wrapping when narrow */}
          <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
            <div className="flex items-center gap-3">
              <PoolPairCell tokenA={pool.tokenA} tokenB={pool.tokenB} />
            </div>
            <span className="rounded-[4px] border border-sothoth-500/20 bg-sothoth-600/10 px-2 py-[3px] text-[12px] font-semibold tracking-[0.12em] text-sothoth-200 uppercase">
              {formatProtocolLabel(pool.protocol)}
            </span>
          </div>

          {/* Short address with copy */}
          <div className="mt-3 flex items-center gap-2 text-[14px] text-slate-400">
            <span className="font-mono">
              {formatShortAddress(pool.poolAddress)}
            </span>
            <CopyButton
              value={pool.poolAddress}
              label={t("copyAddress")}
            />
          </div>
        </div>

        {/* Right — external CTAs */}
        <div className="flex flex-wrap gap-2">
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