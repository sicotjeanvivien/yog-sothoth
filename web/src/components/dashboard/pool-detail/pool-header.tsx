/**
 * Header card for the pool detail page.
 *
 * Shows the pool's identity — address, protocol, mint pair, discovery
 * timestamps. Pure presentational; labels come from the parent via
 * `next-intl`. Server-rendered.
 */

import type { PoolResponse } from "@/lib/api/schema/pool-response";
import { formatAbsolute, formatRelative, type FormatLocale } from "@/lib/format/date";
import { shortenPubkey } from "@/lib/format/pubkey";

export type PoolHeaderLabels = {
  protocol: string;
  pair: string;
  firstSeen: string;
  lastSeen: string;
};

type PoolHeaderProps = {
  pool: PoolResponse;
  labels: PoolHeaderLabels;
  locale: FormatLocale;
  /** Reference "now" used for relative formatting. Defaults to runtime now. */
  now?: Date;
};

export function PoolHeader({ pool, labels, locale, now }: PoolHeaderProps) {
  const firstSeenRelative = formatRelative(pool.first_seen_at, locale, now);
  const lastSeenRelative = formatRelative(pool.last_seen_at, locale, now);

  return (
    <section className="rounded-lg border border-cosmos-700/60 bg-cosmos-900/60 px-6 py-6 shadow-[0_0_40px_-12px_rgba(124,58,237,0.25)] backdrop-blur-sm">
      {/* Address — primary identifier */}
      <div className="flex flex-wrap items-baseline gap-3">
        <h1
          className="font-mono text-xl text-sothoth-400 break-all sm:text-2xl"
          title={pool.pool_address}
        >
          {shortenPubkey(pool.pool_address)}
        </h1>
        <ProtocolBadge protocol={pool.protocol} label={labels.protocol} />
      </div>

      {/* Metadata grid — pair + timestamps */}
      <dl className="mt-6 grid grid-cols-1 gap-x-8 gap-y-4 sm:grid-cols-3">
        <MetaItem label={labels.pair}>
          <span className="flex flex-col font-mono text-xs">
            <span className="text-slate-300" title={pool.token_a_mint}>
              {shortenPubkey(pool.token_a_mint)}
            </span>
            <span className="text-slate-500" title={pool.token_b_mint}>
              {shortenPubkey(pool.token_b_mint)}
            </span>
          </span>
        </MetaItem>

        <MetaItem label={labels.firstSeen}>
          <span title={formatAbsolute(pool.first_seen_at) ?? ""}>
            {firstSeenRelative ?? "—"}
          </span>
        </MetaItem>

        <MetaItem label={labels.lastSeen}>
          <span title={formatAbsolute(pool.last_seen_at) ?? ""}>
            {lastSeenRelative ?? "—"}
          </span>
        </MetaItem>
      </dl>
    </section>
  );
}

// ── Sub-components ─────────────────────────────────────────────────────

function MetaItem({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <dt className="text-[10px] uppercase tracking-[0.18em] text-slate-500">
        {label}
      </dt>
      <dd className="mt-1 text-sm text-slate-300">{children}</dd>
    </div>
  );
}

function ProtocolBadge({ protocol, label }: { protocol: string; label: string }) {
  return (
    <span
      className="inline-flex items-center rounded-full border border-eldritch-600/40 bg-eldritch-600/10 px-2 py-0.5 font-mono text-xs text-eldritch-400"
      title={label}
    >
      {protocol}
    </span>
  );
}