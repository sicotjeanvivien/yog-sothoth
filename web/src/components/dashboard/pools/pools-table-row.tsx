/**
 * One row in the pools table.
 *
 * The row is an `<a>` (via next-intl's locale-aware `<Link>`)
 * directly wrapping its cells. No native `<table>` nesting problem to
 * dodge, no Client Component required: the prefetch and accessibility
 * of `<Link>` work out of the box.
 *
 * Eight cells: pair, signal indicator, protocol, fee, TVL, 24h volume,
 * first seen, last seen. Cell widths are governed by the `GRID_COLS`
 * template shared with the header, so the columns stay aligned
 * regardless of content.
 *
 * USD values render `—` when null — that path exists when TVL or
 * volume cannot be computed for the pool (missing prices, no
 * recent swaps, etc.). The format helper handles the null check.
 *
 * Locale is passed in by the parent so this stays a Server
 * Component without calling `getLocale` per row.
 */

import { Link } from "@/i18n/navigation";

import type { PoolResponse } from "@/lib/api/schema/pool";
import { formatFeeBps } from "@/lib/format/format-fee";
import { formatProtocolLabel } from "@/lib/format/format-protocol";
import { formatRelativeTime } from "@/lib/format/format-relative-time";
import { formatUsdCompact } from "@/lib/format/format-usd";
import { worstSeverity } from "@/lib/signals/worst-severity";

import { PoolPairCell } from "./pool-pair-cell";
import { PoolSignalsCell } from "./pool-signals-cell";
import { GRID_COLS } from "./pools-table";

/** Labels the signal cell needs, resolved once by the parent table. */
export type SignalCellLabels = {
  /** Localized detector tag; falls back to the raw detector name. */
  tagFor: (detector: string) => string;
  /** Accessible name of the indicator, given the signal count. */
  ariaFor: (count: number) => string;
  /** Popover heading. */
  title: string;
};

const CELL_CLASS =
  "px-4 py-3 text-[14px] text-slate-300 align-middle whitespace-nowrap flex items-center";

const CELL_NUMERIC_CLASS = `${CELL_CLASS} justify-end font-mono`;

export function PoolsTableRow({
  pool,
  locale,
  signalLabels,
}: {
  pool: PoolResponse;
  locale: string;
  signalLabels: SignalCellLabels;
}) {
  const worst = worstSeverity(pool.signals24h);

  return (
    <Link
      role="row"
      href={`/pools/${pool.poolAddress}`}
      className={`grid ${GRID_COLS} cursor-pointer border-b border-sothoth-500/10 transition-colors last:border-b-0 hover:bg-sothoth-500/[0.04]`}
    >
      <div role="cell" className={CELL_CLASS}>
        <PoolPairCell tokenA={pool.tokenA} tokenB={pool.tokenB} />
      </div>
      {/* Signal indicator — empty cell (grid alignment) when the pool
          emitted nothing in the window. */}
      <div role="cell" className={CELL_CLASS}>
        {worst !== null && (
          <PoolSignalsCell
            alertsHref={`/pools/${pool.poolAddress}?tab=alerts`}
            ariaLabel={signalLabels.ariaFor(pool.signals24h.length)}
            title={signalLabels.title}
            worst={worst}
            items={pool.signals24h.map((signal) => ({
              severity: signal.severity,
              label: signalLabels.tagFor(signal.detector),
            }))}
          />
        )}
      </div>
      <div role="cell" className={CELL_CLASS}>
        <span className="text-slate-400">
          {formatProtocolLabel(pool.protocol)}
        </span>
      </div>
      <div role="cell" className={CELL_NUMERIC_CLASS}>
        {formatFeeBps(pool.feeBps)}
      </div>
      <div role="cell" className={CELL_NUMERIC_CLASS}>
        {formatUsdCompact(pool.tvlUsd)}
      </div>
      <div role="cell" className={CELL_NUMERIC_CLASS}>
        {formatUsdCompact(pool.volume24hUsd)}
      </div>
      {/* suppressHydrationWarning: relative to now, so the SSR text can
          legitimately lag the client by a minute boundary. */}
      <div role="cell" className={CELL_CLASS}>
        <time
          dateTime={pool.firstSeenAt}
          className="text-slate-400"
          suppressHydrationWarning
        >
          {formatRelativeTime(pool.firstSeenAt, locale)}
        </time>
      </div>
      <div role="cell" className={CELL_CLASS}>
        <time
          dateTime={pool.lastSeenAt}
          className="text-slate-400"
          suppressHydrationWarning
        >
          {formatRelativeTime(pool.lastSeenAt, locale)}
        </time>
      </div>
    </Link>
  );
}
