/**
 * Pool detail page — "Recent liquidity events" block.
 *
 * Same CSS-grid + ARIA structure as the swaps block (see
 * `pool-detail-swaps.tsx` for the rationale). Six columns:
 *
 *   Time       — relative ("2 min ago")
 *   Kind       — coloured badge: "Add" (green) / "Remove" (red)
 *   Amount A   — `amountA` in human units + token A symbol
 *   Amount B   — `amountB` in human units + token B symbol
 *   Value (USD)— `valueUsd`, trade-time value of the event ("—" if unknown)
 *   Action     — copy the signature / open the tx on Solscan
 *
 * Unlike a swap (which has an "in" and an "out" side), a liquidity
 * event touches both tokens together: adding 5 SOL + 100 USDC, or
 * removing 2 SOL + 40 USDC. Both amounts are displayed in parallel,
 * with no in/out logic.
 *
 * Only the first page (limit 20) is fetched in this commit; the
 * Load-more pagination lands later, alongside the swaps one.
 *
 * Rows are NOT clickable — liquidity events don't have their own
 * page; the only useful affordances are the per-row actions (copy
 * signature, open on Solscan) in the last column.
 */

import { getTranslations } from "next-intl/server";

import type { PoolResponse } from "@/lib/api/schema/pool";
import type { LiquidityEventResponse } from "@/lib/api/schema/liquidity-event";

import { SolscanIcon } from "@/components/shared/icon";
import { CopyButton } from "@/components/shared/copy-button";

import { formatRelativeTime } from "@/lib/format/format-relative-time";
import { formatTokenAmount } from "@/lib/format/format-token-amount";
import { formatUsd } from "@/lib/format/format-usd";
import { TokenResponse } from "@/lib/api/schema/token";

// ── Tailwind class fragments ─────────────────────────────────────────

const SECTION_CLASS = "px-6 lg:px-10";

const CARD_CLASS =
  "rounded-[8px] border border-sothoth-500/15 bg-cosmos-900/40";

const TITLE_BAR_CLASS =
  "flex items-center justify-between border-b border-sothoth-500/20 px-6 py-4";

const SECTION_TITLE_CLASS =
  "text-[11px] font-semibold tracking-[0.2em] text-slate-400 uppercase";

const TABLE_WRAPPER_CLASS = "overflow-x-auto";

const GRID_COLS =
  "grid-cols-[minmax(110px,1fr)_minmax(100px,0.8fr)_minmax(140px,1fr)_minmax(140px,1fr)_minmax(120px,1fr)_minmax(96px,auto)]";

const HEAD_CELL_CLASS =
  "px-4 py-3 text-left text-[11px] font-semibold tracking-[0.2em] text-slate-400 uppercase whitespace-nowrap";

const CELL_CLASS =
  "px-4 py-3 text-[13px] text-slate-300 align-middle whitespace-nowrap flex items-center";

const CELL_MONO_CLASS = `${CELL_CLASS} font-mono`;

const ACTION_LINK_CLASS =
  "inline-flex h-6 w-6 items-center justify-center rounded-[3px] text-slate-400 transition-colors hover:bg-sothoth-500/15 hover:text-sothoth-300";

// Badge styles — emerald for add (capital inflow), rose for remove
// (capital outflow). Translucent backgrounds so the badge sits
// gently on the dark surface without overpowering the row.
const BADGE_ADD_CLASS =
  "inline-flex items-center rounded-[3px] border border-emerald-500/30 bg-emerald-500/10 px-2 py-[2px] text-[11px] font-semibold tracking-[0.1em] text-emerald-300 uppercase";

const BADGE_REMOVE_CLASS =
  "inline-flex items-center rounded-[3px] border border-rose-500/30 bg-rose-500/10 px-2 py-[2px] text-[11px] font-semibold tracking-[0.1em] text-rose-300 uppercase";

// ── Component ─────────────────────────────────────────────────────────

export async function PoolDetailLiquidity({
  pool,
  events,
  locale,
}: {
  pool: PoolResponse;
  events: LiquidityEventResponse[];
  locale: string;
}) {
  const t = await getTranslations("Dashboard.PoolDetail.liquidity");
  
  return (
    <section className={`mt-6 ${SECTION_CLASS}`}>
      <div className={CARD_CLASS}>
        <div className={TITLE_BAR_CLASS}>
          <h2 className={SECTION_TITLE_CLASS}>{t("title")}</h2>
        </div>

        {events.length === 0 ? (
          <EmptyState message={t("empty")} />
        ) : (
          <div className={TABLE_WRAPPER_CLASS}>
            <div role="table" className="min-w-[880px]">
              <div role="rowgroup" className="border-b border-sothoth-500/20">
                <div role="row" className={`grid ${GRID_COLS}`}>
                  <div role="columnheader" className={HEAD_CELL_CLASS}>
                    {t("time")}
                  </div>
                  <div role="columnheader" className={HEAD_CELL_CLASS}>
                    {t("kind")}
                  </div>
                  <div role="columnheader" className={HEAD_CELL_CLASS}>
                    {t("amountA")}
                  </div>
                  <div role="columnheader" className={HEAD_CELL_CLASS}>
                    {t("amountB")}
                  </div>
                  <div role="columnheader" className={HEAD_CELL_CLASS}>
                    {t("valueUsd")}
                  </div>
                  <div role="columnheader" className={HEAD_CELL_CLASS}>
                    {t("action")}
                  </div>
                </div>
              </div>

              <div role="rowgroup">
                {events.map((event) => (
                  <LiquidityRow
                    key={event.signature}
                    event={event}
                    tokenA={pool.tokenA}
                    tokenB={pool.tokenB}
                    locale={locale}
                    addLabel={t("add")}
                    removeLabel={t("remove")}
                    copyLabel={t("copySignature")}
                    solscanLabel={t("viewOnSolscan")}
                  />
                ))}
              </div>
            </div>
          </div>
        )}
      </div>
    </section>
  );
}

// ── Sub-components ───────────────────────────────────────────────────

function LiquidityRow({
  event,
  tokenA,
  tokenB,
  locale,
  addLabel,
  removeLabel,
  copyLabel,
  solscanLabel,
}: {
  event: LiquidityEventResponse;
  tokenA: TokenResponse;
  tokenB: TokenResponse;
  locale: string;
  addLabel: string;
  removeLabel: string;
  copyLabel: string;
  solscanLabel: string;
}) {
  const isAdd = event.liquidityEventKind === "add";
  const solscanUrl = `https://solscan.io/tx/${event.signature}`;

  return (
    <div
      role="row"
      className={`grid ${GRID_COLS} border-b border-sothoth-500/10 last:border-b-0`}
    >
      <div role="cell" className={CELL_CLASS}>
        <time dateTime={event.timestamp} className="text-slate-400">
          {formatRelativeTime(event.timestamp, locale)}
        </time>
      </div>

      <div role="cell" className={CELL_CLASS}>
        <span className={isAdd ? BADGE_ADD_CLASS : BADGE_REMOVE_CLASS}>
          {isAdd ? addLabel : removeLabel}
        </span>
      </div>

      <div role="cell" className={CELL_MONO_CLASS}>
        {formatTokenAmount(event.amountA, tokenA.decimals, tokenA.symbol)}
      </div>

      <div role="cell" className={CELL_MONO_CLASS}>
        {formatTokenAmount(event.amountB, tokenB.decimals, tokenB.symbol)}
      </div>

      <div role="cell" className={CELL_MONO_CLASS}>
        {formatUsd(event.valueUsd)}
      </div>

      <div role="cell" className={CELL_CLASS}>
        <div className="flex items-center gap-1">
          <CopyButton value={event.signature} label={copyLabel} />
          <a
            href={solscanUrl}
            target="_blank"
            rel="noopener noreferrer"
            className={ACTION_LINK_CLASS}
            aria-label={solscanLabel}
            title={solscanLabel}
          >
            <SolscanIcon size={16} />
          </a>
        </div>
      </div>
    </div>
  );
}

function EmptyState({ message }: { message: string }) {
  return (
    <div className="flex flex-col items-center px-6 py-12 text-center">
      <p className="max-w-[52ch] text-[13px] leading-[1.6] text-slate-400">
        {message}
      </p>
    </div>
  );
}