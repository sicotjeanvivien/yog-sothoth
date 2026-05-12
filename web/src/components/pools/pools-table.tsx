/**
 * Dense table listing the pools observed by yog-sothoth.
 *
 * Pure presentational component, rendered server-side. Layout choices:
 *
 *   - Two-tier row: primary tier shows the pool address + protocol
 *     badge, secondary tier under it shows the mint pair in muted
 *     typography. Avoids horizontal cramping on narrow screens while
 *     keeping enough density for power users.
 *   - `first_seen_at` as the primary relative time (the discovery
 *     timestamp), `last_seen_at` as a UTC absolute below it in muted
 *     style. Mirrors the temporal hierarchy of the underlying data.
 *   - Subtle hover halo via `hover:bg-cosmos-700/40` for visual
 *     feedback without disrupting the dense layout.
 *
 * Header labels and the empty cell fallback come from the parent via
 * `next-intl` translations, kept out of the component to keep it
 * locale-agnostic and easy to reuse.
 */

import type { PoolResponse } from "@/lib/api/schemas";
import { formatAbsolute, formatRelative, type FormatLocale } from "@/lib/format/date";
import { shortenPubkey } from "@/lib/format/pubkey";

export type PoolsTableLabels = {
  address: string;
  protocol: string;
  pair: string;
  firstSeen: string;
  lastSeen: string;
};

type PoolsTableProps = {
  pools: PoolResponse[];
  labels: PoolsTableLabels;
  locale: FormatLocale;
  /** Reference "now" used for relative formatting. Defaults to runtime now. */
  now?: Date;
};

export function PoolsTable({ pools, labels, locale, now }: PoolsTableProps) {
  return (
    <div className="overflow-hidden rounded-lg border border-cosmos-700/60 bg-cosmos-900/60 shadow-[0_0_40px_-12px_rgba(124,58,237,0.25)] backdrop-blur-sm">
      <table className="w-full border-collapse text-left text-sm">
        <thead>
          <tr className="border-b border-cosmos-700/60 bg-cosmos-800/40">
            <Th>{labels.address}</Th>
            <Th>{labels.protocol}</Th>
            <Th>{labels.pair}</Th>
            <Th>{labels.firstSeen}</Th>
            <Th className="text-right">{labels.lastSeen}</Th>
          </tr>
        </thead>
        <tbody>
          {pools.map((pool) => (
            <Row
              key={pool.address}
              pool={pool}
              locale={locale}
              {...(now !== undefined && { now })}
            />
          ))}
        </tbody>
      </table>
    </div>
  );
}

// ── Sub-components ─────────────────────────────────────────────────────

function Th({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <th
      className={`px-4 py-3 text-xs font-medium uppercase tracking-[0.18em] text-slate-400 ${className ?? ""}`}
    >
      {children}
    </th>
  );
}

function Row({
  pool,
  locale,
  now,
}: {
  pool: PoolResponse;
  locale: FormatLocale;
  now?: Date;
}) {
  const firstSeenRelative = formatRelative(pool.first_seen_at, locale, now);
  const lastSeenAbsolute = formatAbsolute(pool.last_seen_at);

  return (
    <tr className="border-b border-cosmos-700/40 transition-colors last:border-b-0 hover:bg-cosmos-700/30">
      {/* Address — primary identifier, slightly more visual weight */}
      <td className="px-4 py-3 font-mono text-sothoth-400">
        <span title={pool.address}>{shortenPubkey(pool.address)}</span>
      </td>

      {/* Protocol — discreet pill */}
      <td className="px-4 py-3">
        <ProtocolBadge protocol={pool.protocol} />
      </td>

      {/* Mint pair — secondary, muted */}
      <td className="px-4 py-3 font-mono text-xs text-slate-400">
        <div className="flex flex-col">
          <span title={pool.token_a_mint}>{shortenPubkey(pool.token_a_mint)}</span>
          <span title={pool.token_b_mint} className="text-slate-500">
            {shortenPubkey(pool.token_b_mint)}
          </span>
        </div>
      </td>

      {/* First seen — relative, with absolute on hover */}
      <td className="px-4 py-3 text-slate-300" title={formatAbsolute(pool.first_seen_at) ?? ""}>
        {firstSeenRelative ?? "—"}
      </td>

      {/* Last seen — absolute UTC, right-aligned for scanning */}
      <td className="px-4 py-3 text-right font-mono text-xs text-slate-400">
        {lastSeenAbsolute ?? "—"}
      </td>
    </tr>
  );
}

function ProtocolBadge({ protocol }: { protocol: string }) {
  // Single style for now — every observed pool is DAMM v2. The branch
  // will grow as Phase 2 / 3 protocols come online.
  return (
    <span className="inline-flex items-center rounded-full border border-eldritch-600/40 bg-eldritch-600/10 px-2 py-0.5 font-mono text-xs text-eldritch-400">
      {protocol}
    </span>
  );
}