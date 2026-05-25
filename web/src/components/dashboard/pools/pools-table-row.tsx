/**
 * One row in the pools table.
 *
 * Three columns: pair, protocol, last seen (relative time).
 * TVL and 24h volume will be added once the API exposes them.
 *
 * The entire row is the link to the pool's detail page —
 * implemented by wrapping the row in next-intl's `<Link>` with
 * `display: table-row`. This keeps the row semantically a `<tr>`
 * while making the whole surface clickable.
 *
 * Locale is passed in by the parent so this stays a Server
 * Component without calling `getLocale` per row.
 */

import { Link } from "@/i18n/navigation";

import type { PoolResponse } from "@/lib/api/schema/pool";
import { formatProtocolLabel } from "@/lib/format/format-protocol";
import { formatRelativeTime } from "@/lib/format/format-relative-time";

import { PoolPairCell } from "./pool-pair-cell";

const CELL_CLASS =
  "px-4 py-3 text-[14px] text-slate-300 align-middle whitespace-nowrap";

export function PoolsTableRow({
  pool,
  locale,
}: {
  pool: PoolResponse;
  locale: string;
}) {
  return (
    <Link
      href={`/pools/${pool.pool_address}`}
      className="table-row cursor-pointer border-b border-sothoth-500/10 transition-colors last:border-b-0 hover:bg-sothoth-500/[0.04]"
    >
      <td className={CELL_CLASS}>
        <PoolPairCell tokenA={pool.token_a} tokenB={pool.token_b} />
      </td>
      <td className={CELL_CLASS}>
        <span className="text-slate-400">
          {formatProtocolLabel(pool.protocol)}
        </span>
      </td>
      <td className={`${CELL_CLASS} text-slate-400`}>
        <time dateTime={pool.last_seen_at}>
          {formatRelativeTime(pool.last_seen_at, locale)}
        </time>
      </td>
    </Link>
  );
}