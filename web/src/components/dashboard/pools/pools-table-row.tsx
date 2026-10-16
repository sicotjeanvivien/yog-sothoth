/**
 * One pool row, for `/pools` (server) and `/watchlist` (client): labels and `locale` come as
 * plain values. ⚠️ Each data cell is its own `<Link>`; the actions cell is a sibling, never
 * nested in one.
 */

import { Link } from "@/i18n/navigation";

import type { PoolResponse } from "@/lib/api/schema/zod.gen";
import { formatFeeBps } from "@/lib/format/format-fee";
import { formatRelativeTime } from "@/lib/format/format-relative-time";
import { formatUsdCompact } from "@/lib/format/format-usd";
import { volumeCoverage } from "@/lib/coverage/volume-coverage";
import { worstSeverity } from "@/lib/signals/worst-severity";

import { PoolPairCell } from "./pool-pair-cell";
import { PoolRowActions } from "./pool-row-actions";
import { PoolSignalsCell } from "./pool-signals-cell";
import { ProtocolBadge } from "./protocol-badge";
import {
  CELL_CLASS,
  CELL_NUMERIC_CLASS,
  GRID_COLS,
  type CoverageLabel,
  type SignalCellLabels,
} from "./pools-table-shared";
import { VolumeCoverageMark } from "./volume-coverage-mark";

export type { CoverageLabel, SignalCellLabels };

export function PoolsTableRow({
  pool,
  locale,
  signalLabels,
  coverageLabel,
}: {
  pool: PoolResponse;
  locale: string;
  signalLabels: SignalCellLabels;
  coverageLabel: CoverageLabel;
}) {
  const worst = worstSeverity(pool.signals24h);
  // Also when nothing was valued: the `—` would otherwise read as "did not trade".
  const coverage = volumeCoverage(pool);
  const href = `/pools/${pool.poolAddress}`;

  return (
    <div
      role="row"
      className={`grid ${GRID_COLS} border-b border-sothoth-500/10 transition-colors last:border-b-0 hover:bg-sothoth-500/[0.04]`}
    >
      <Link role="cell" href={href} className={`${CELL_CLASS} min-w-0`}>
        <PoolPairCell tokenA={pool.tokenA} tokenB={pool.tokenB} />
      </Link>

      {/* Not a Link: the cell carries its own alerts link. */}
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
        <VolumeCoverageMark coverage={coverage} labelFor={coverageLabel} />
      </Link>

      {/* Relative to now: the SSR text may lag the client by a minute. */}
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
