/**
 * One row in the pool table — shared by `/pools` (server-rendered) and
 * `/watchlist` (client-rendered).
 *
 * Prop-driven and free of server-only imports, so it renders in either tree:
 * the parent resolves `signalLabels` (server via `getTranslations`, client via
 * `useTranslations`) and passes them, plus `locale`, as plain values.
 *
 * Nine cells: pair · signal indicator · protocol · fee · TVL · 24h volume ·
 * first seen · last seen · actions. The eight data cells are individual
 * `<Link>`s to the pool detail; the trailing actions cell is a sibling (never
 * nested in a link) so its copy/Solscan/watchlist controls stay valid and
 * clickable. Widths come from the shared `GRID_COLS`.
 *
 * USD values render `—` when null (missing prices / no recent swaps); the
 * format helper owns the null check.
 */

import { Link } from "@/i18n/navigation";

import type { PoolResponse } from "@/lib/api/schema/pool";
import { formatFeeBps } from "@/lib/format/format-fee";
import { formatRelativeTime } from "@/lib/format/format-relative-time";
import { formatUsdCompact } from "@/lib/format/format-usd";
import { worstSeverity } from "@/lib/signals/worst-severity";

import { PoolPairCell } from "./pool-pair-cell";
import { PoolRowActions } from "./pool-row-actions";
import { PoolSignalsCell } from "./pool-signals-cell";
import { ProtocolBadge } from "./protocol-badge";
import {
  CELL_CLASS,
  CELL_NUMERIC_CLASS,
  GRID_COLS,
  type SignalCellLabels,
} from "./pools-table-shared";

export type { SignalCellLabels };

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
  const href = `/pools/${pool.poolAddress}`;

  return (
    <div
      role="row"
      className={`grid ${GRID_COLS} border-b border-sothoth-500/10 transition-colors last:border-b-0 hover:bg-sothoth-500/[0.04]`}
    >
      <Link role="cell" href={href} className={`${CELL_CLASS} min-w-0`}>
        <PoolPairCell tokenA={pool.tokenA} tokenB={pool.tokenB} />
      </Link>

      {/* Signal indicator — empty cell (grid alignment) when the pool emitted
          nothing in the window. Not a Link: it carries its own alerts link. */}
      <div role="cell" className={CELL_CLASS}>
        {worst !== null && (
          <PoolSignalsCell
            alertsHref={`${href}?tab=alerts`}
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

      <Link role="cell" href={href} className={CELL_CLASS}>
        <ProtocolBadge protocol={pool.protocol} />
      </Link>
      <Link role="cell" href={href} className={CELL_NUMERIC_CLASS}>
        {formatFeeBps(pool.feeBps)}
      </Link>
      <Link role="cell" href={href} className={CELL_NUMERIC_CLASS}>
        {formatUsdCompact(pool.tvlUsd)}
      </Link>
      <Link role="cell" href={href} className={CELL_NUMERIC_CLASS}>
        {formatUsdCompact(pool.volume24hUsd)}
      </Link>

      {/* suppressHydrationWarning: relative to now, so the SSR text can
          legitimately lag the client by a minute boundary. */}
      <Link role="cell" href={href} className={CELL_CLASS}>
        <time
          dateTime={pool.firstSeenAt}
          className="text-slate-400"
          suppressHydrationWarning
        >
          {formatRelativeTime(pool.firstSeenAt, locale, { style: "short" })}
        </time>
      </Link>
      <Link role="cell" href={href} className={CELL_CLASS}>
        <time
          dateTime={pool.lastSeenAt}
          className="text-slate-400"
          suppressHydrationWarning
        >
          {formatRelativeTime(pool.lastSeenAt, locale, { style: "short" })}
        </time>
      </Link>

      <div role="cell" className={`${CELL_CLASS} justify-end`}>
        <PoolRowActions address={pool.poolAddress} />
      </div>
    </div>
  );
}
