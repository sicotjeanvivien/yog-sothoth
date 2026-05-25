/**
 * One row in the pools table.
 *
 * The row is an `<a>` (via next-intl's locale-aware `<Link>`)
 * directly wrapping its three cells. No native `<table>` nesting
 * problem to dodge, no Client Component required: the prefetch and
 * accessibility of `<Link>` work out of the box.
 *
 * Three cells: pair, protocol, last seen. Cell widths are governed
 * by the `GRID_COLS` template shared with the header, so the
 * columns stay aligned regardless of content.
 *
 * Locale is passed in by the parent so this stays a Server
 * Component without calling `getLocale` per row.
 */

import { Link } from "@/i18n/navigation";

import type { PoolResponse } from "@/lib/api/schema/pool";
import { formatProtocolLabel } from "@/lib/format/format-protocol";
import { formatRelativeTime } from "@/lib/format/format-relative-time";

import { PoolPairCell } from "./pool-pair-cell";
import { GRID_COLS } from "./pools-table";

const CELL_CLASS =
  "px-4 py-3 text-[14px] text-slate-300 align-middle whitespace-nowrap flex items-center";

export function PoolsTableRow({
  pool,
  locale,
}: {
  pool: PoolResponse;
  locale: string;
}) {
  return (
    <Link
      role="row"
      href={`/pools/${pool.pool_address}`}
      className={`grid ${GRID_COLS} cursor-pointer border-b border-sothoth-500/10 transition-colors last:border-b-0 hover:bg-sothoth-500/[0.04]`}
    >
      <div role="cell" className={CELL_CLASS}>
        <PoolPairCell tokenA={pool.token_a} tokenB={pool.token_b} />
      </div>
      <div role="cell" className={CELL_CLASS}>
        <span className="text-slate-400">
          {formatProtocolLabel(pool.protocol)}
        </span>
      </div>
      <div role="cell" className={CELL_CLASS}>
        <time dateTime={pool.last_seen_at} className="text-slate-400">
          {formatRelativeTime(pool.last_seen_at, locale)}
        </time>
      </div>
    </Link>
  );
}