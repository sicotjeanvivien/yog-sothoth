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
 *   - Each row is a clickable link to the pool detail page. We anchor
 *     at the cell level rather than wrapping the <tr> (which would be
 *     invalid HTML); the user perceives a full-row click affordance.
 *
 * Header labels and the empty cell fallback come from the parent via
 * `next-intl` translations, kept out of the component to keep it
 * locale-agnostic and easy to reuse.
 */

import type { PoolResponse } from "@/lib/api/schema/pool-response";
import { formatAbsolute, formatRelative, type FormatLocale } from "@/lib/format/date";
import { shortenPubkey } from "@/lib/format/pubkey";

export type PoolsTableLabels = {
  pool_address: string;
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
            <Th>{labels.pool_address}</Th>
            <Th>{labels.protocol}</Th>
            <Th>{labels.pair}</Th>
            <Th>{labels.firstSeen}</Th>
            <Th className="text-right">{labels.lastSeen}</Th>
          </tr>
        </thead>
        <tbody>
          {pools.map((pool) => (
            <Row
              key={pool.pool_address}
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

/**
 * Wrap a cell's inner content in an anchor tag. Block-level so the
 * entire cell becomes the clickable hit target, not just the text.
 *
 * Anchoring at the cell level (rather than wrapping the <tr>) keeps
 * the markup valid HTML while still giving the user a full-row click
 * affordance.
 */
function CellLink({
  href,
  className,
  title,
  children,
}: {
  href: string;
  className?: string;
  title?: string;
  children: React.ReactNode;
}) {
  return (
    <a
      href={href}
      title={title}
      className={`block focus:outline-none focus:ring-1 focus:ring-sothoth-500/60 focus:rounded-sm ${className ?? ""}`}
    >
      {children}
    </a>
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
  const detailHref = `/${locale}/pools/${pool.pool_address}`;

  return (
    <tr className="border-b border-cosmos-700/40 transition-colors last:border-b-0 hover:bg-cosmos-700/30">
      <td className="px-4 py-3 font-mono text-sothoth-400">
        <CellLink href={detailHref} title={pool.pool_address}>
          {shortenPubkey(pool.pool_address)}
        </CellLink>
      </td>

      <td className="px-4 py-3">
        <CellLink href={detailHref}>
          <ProtocolBadge protocol={pool.protocol} />
        </CellLink>
      </td>

      <td className="px-4 py-3 font-mono text-xs text-slate-400">
        <CellLink href={detailHref}>
          <div className="flex flex-col">
            <span title={pool.token_a_mint}>{shortenPubkey(pool.token_a_mint)}</span>
            <span title={pool.token_b_mint} className="text-slate-500">
              {shortenPubkey(pool.token_b_mint)}
            </span>
          </div>
        </CellLink>
      </td>

      <td className="px-4 py-3 text-slate-300" title={formatAbsolute(pool.first_seen_at) ?? ""}>
        <CellLink href={detailHref}>{firstSeenRelative ?? "—"}</CellLink>
      </td>

      <td className="px-4 py-3 text-right font-mono text-xs text-slate-400">
        <CellLink href={detailHref}>{lastSeenAbsolute ?? "—"}</CellLink>
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