/**
 * The pool detail header: back link, pair and protocol, watchlist and external actions. A
 * missing symbol shows as nothing; the Pool info block shows the mints.
 */

import { getTranslations } from "next-intl/server";

import { Link } from "@/i18n/navigation";

import { ArrowLeftIcon, ExternalLinkIcon } from "@/components/shared/icon";

import type { PoolResponse } from "@/lib/api/schema/zod.gen";
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
      <Link
        href="/pools"
        className="inline-flex items-center gap-1.5 text-[14px] text-slate-400 transition-colors hover:text-slate-200"
      >
        <ArrowLeftIcon size={14} />
        {t("backToList")}
      </Link>

      <div className="mt-6 grid grid-cols-1 items-start gap-6 lg:grid-cols-[1fr_auto]">
        <div>
          <div className="flex items-center gap-3">
            <PoolPairCell tokenA={pool.tokenA} tokenB={pool.tokenB} />
          </div>
          <div className="mt-3 flex items-center gap-2 text-[14px] text-slate-400">
            <ProtocolBadge protocol={pool.protocol} />
          </div>
        </div>

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