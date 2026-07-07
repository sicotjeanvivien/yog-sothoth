/**
 * Pool detail page — "Recent swaps" block.
 *
 * Renders the most recent page of swap events for the pool as a
 * CSS-grid table with ARIA roles (same pattern as `/pools` — see
 * `pools-table.tsx` for the rationale). Six columns:
 *
 *   Time     — relative ("2 min ago")
 *   Direction — "SOL → USDC" from `tradeDirection`
 *   Amount in — what the trader sent, in human units + symbol
 *   Amount out — what the trader received
 *   Fee       — the claiming (LP) fee, in the fee token (`feeTokenIsA`)
 *   Action    — copy the signature / open the tx on Solscan
 *
 * No pagination in this commit: only the first page is shown.
 * Interactive pagination (Load more) lives in a separate change.
 *
 * Rows are NOT clickable — swap events don't have their own page;
 * the only useful affordances are the per-row actions (copy
 * signature, open on Solscan) in the last column.
 *
 * The empty state replaces the table when no swaps have been
 * observed yet for the pool.
 */

import { getTranslations } from "next-intl/server";

import { TokenResponse } from "@/lib/api/schema/token";
import type { PoolResponse } from "@/lib/api/schema/pool";
import type { SwapEventResponse } from "@/lib/api/schema/swap-event";

import { SolscanIcon } from "@/components/shared/icon";
import { CopyButton } from "@/components/shared/copy-button";

import { formatRelativeTime } from "@/lib/format/format-relative-time";
import { formatTokenAmount } from "@/lib/format/format-token-amount";

// ── Tailwind class fragments ─────────────────────────────────────────

const SECTION_CLASS = "px-6 lg:px-10";

const CARD_CLASS =
  "rounded-[8px] border border-sothoth-500/15 bg-cosmos-900/40";

const TITLE_BAR_CLASS =
  "flex items-center justify-between border-b border-sothoth-500/20 px-6 py-4";

const SECTION_TITLE_CLASS =
  "text-[12px] font-semibold tracking-[0.2em] text-slate-400 uppercase";

const TABLE_WRAPPER_CLASS = "overflow-x-auto";

const GRID_COLS =
  "grid-cols-[minmax(110px,0.9fr)_minmax(120px,0.9fr)_minmax(150px,1.3fr)_minmax(150px,1.3fr)_minmax(130px,1.1fr)_minmax(96px,auto)]";

const HEAD_CELL_CLASS =
  "px-4 py-3 text-left text-[12px] font-semibold tracking-[0.2em] text-slate-400 uppercase whitespace-nowrap";

const CELL_CLASS =
  "px-4 py-3 text-[14px] text-slate-300 align-middle whitespace-nowrap flex items-center";

const CELL_MONO_CLASS = `${CELL_CLASS} font-mono`;

const ACTION_LINK_CLASS =
  "inline-flex h-6 w-6 items-center justify-center rounded-[3px] text-slate-400 transition-colors hover:bg-sothoth-500/15 hover:text-sothoth-300";

// ── Component ─────────────────────────────────────────────────────────

export async function PoolDetailSwaps({
  pool,
  swaps,
  locale,
}: {
  pool: PoolResponse;
  swaps: SwapEventResponse[];
  locale: string;
}) {
  const t = await getTranslations("Dashboard.PoolDetail.swaps");

  return (
    <section className={`mt-6 ${SECTION_CLASS}`}>
      <div className={CARD_CLASS}>
        <div className={TITLE_BAR_CLASS}>
          <h2 className={SECTION_TITLE_CLASS}>{t("title")}</h2>
        </div>

        {swaps.length === 0 ? (
          <EmptyState message={t("empty")} />
        ) : (
          <div className={TABLE_WRAPPER_CLASS}>
            <div role="table" className="min-w-[860px]">
              <div role="rowgroup" className="border-b border-sothoth-500/20">
                <div role="row" className={`grid ${GRID_COLS}`}>
                  <div role="columnheader" className={HEAD_CELL_CLASS}>
                    {t("time")}
                  </div>
                  <div role="columnheader" className={HEAD_CELL_CLASS}>
                    {t("direction")}
                  </div>
                  <div role="columnheader" className={HEAD_CELL_CLASS}>
                    {t("amountIn")}
                  </div>
                  <div role="columnheader" className={HEAD_CELL_CLASS}>
                    {t("amountOut")}
                  </div>
                  <div role="columnheader" className={HEAD_CELL_CLASS}>
                    {t("fee")}
                  </div>
                  <div role="columnheader" className={HEAD_CELL_CLASS}>
                    {t("action")}
                  </div>
                </div>
              </div>

              <div role="rowgroup">
                {swaps.map((swap) => (
                  <SwapRow
                    key={swap.signature}
                    swap={swap}
                    tokenA={pool.tokenA}
                    tokenB={pool.tokenB}
                    locale={locale}
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

function SwapRow({
  swap,
  tokenA,
  tokenB,
  locale,
  copyLabel,
  solscanLabel,
}: {
  swap: SwapEventResponse;
  tokenA: TokenResponse;
  tokenB: TokenResponse;
  locale: string;
  copyLabel: string;
  solscanLabel: string;
}) {
  // trade_direction tells us which side was sent (in) vs received (out).
  //   a_to_b → trader sent amount_a (token A in), received amount_b (token B out)
  //   b_to_a → trader sent amount_b (token B in), received amount_a (token A out)
  const aToB = swap.tradeDirection === "a_to_b";

  const inToken = aToB ? tokenA : tokenB;
  const outToken = aToB ? tokenB : tokenA;
  const inAmount = aToB ? swap.amountA : swap.amountB;
  const outAmount = aToB ? swap.amountB : swap.amountA;

  // The claiming (LP) fee is taken in a single token — `feeTokenIsA`
  // says which — so it's valued in that token's decimals/symbol.
  const feeToken = swap.feeTokenIsA ? tokenA : tokenB;

  const solscanUrl = `https://solscan.io/tx/${swap.signature}`;

  return (
    <div
      role="row"
      className={`grid ${GRID_COLS} border-b border-sothoth-500/10 last:border-b-0`}
    >
      <div role="cell" className={CELL_CLASS}>
        {/* suppressHydrationWarning: relative to now, so the SSR text can
            legitimately lag the client by a minute boundary. */}
        <time
          dateTime={swap.timestamp}
          className="text-slate-400"
          suppressHydrationWarning
        >
          {formatRelativeTime(swap.timestamp, locale)}
        </time>
      </div>

      <div role="cell" className={CELL_CLASS}>
        <span className="font-medium text-slate-100">
          {inToken.symbol ?? "?"}
        </span>
        <span className="mx-2 text-slate-500">→</span>
        <span className="font-medium text-slate-100">
          {outToken.symbol ?? "?"}
        </span>
      </div>

      <div role="cell" className={CELL_MONO_CLASS}>
        {formatTokenAmount(inAmount, inToken.decimals, inToken.symbol)}
      </div>

      <div role="cell" className={CELL_MONO_CLASS}>
        {formatTokenAmount(outAmount, outToken.decimals, outToken.symbol)}
      </div>

      <div role="cell" className={CELL_MONO_CLASS}>
        {formatTokenAmount(
          swap.claimingFee,
          feeToken.decimals,
          feeToken.symbol,
        )}
      </div>

      <div role="cell" className={CELL_CLASS}>
        <div className="flex items-center gap-1">
          <CopyButton value={swap.signature} label={copyLabel} />
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
      <p className="max-w-[52ch] text-[14px] leading-[1.6] text-slate-400">
        {message}
      </p>
    </div>
  );
}
