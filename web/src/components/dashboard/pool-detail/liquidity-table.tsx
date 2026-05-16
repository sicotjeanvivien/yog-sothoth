/**
 * Dense table listing liquidity events on a pool, ordered most-recent
 * first. Both add and remove operations share the same shape, with
 * the kind shown as a coloured badge (green for add, red for remove).
 *
 * Columns:
 *   - Time (relative + absolute on hover)
 *   - Kind (Add / Remove)
 *   - Amount A / Amount B
 *   - Owner (short pubkey)
 *   - Signature (short)
 *
 * The position is intentionally NOT shown — at this density it would
 * push the table beyond a 1024 width. A future per-event detail view
 * will surface it.
 */

import type { LiquidityEventResponse } from "@/lib/api/schemas";
import { formatAbsolute, formatRelative, type FormatLocale } from "@/lib/format/date";
import { shortenPubkey } from "@/lib/format/pubkey";

export type LiquidityTableLabels = {
  time: string;
  kind: string;
  amountA: string;
  amountB: string;
  owner: string;
  signature: string;
  kindAdd: string;
  kindRemove: string;
};

type LiquidityTableProps = {
  events: LiquidityEventResponse[];
  labels: LiquidityTableLabels;
  locale: FormatLocale;
  now?: Date;
};

export function LiquidityTable({ events, labels, locale, now }: LiquidityTableProps) {
  return (
    <table className="w-full border-collapse text-left text-sm">
      <thead>
        <tr className="border-b border-cosmos-700/60">
          <Th>{labels.time}</Th>
          <Th>{labels.kind}</Th>
          <Th className="text-right">{labels.amountA}</Th>
          <Th className="text-right">{labels.amountB}</Th>
          <Th>{labels.owner}</Th>
          <Th className="text-right">{labels.signature}</Th>
        </tr>
      </thead>
      <tbody>
        {events.map((event) => (
          <Row
            key={event.signature}
            event={event}
            labels={labels}
            locale={locale}
            {...(now !== undefined && { now })}
          />
        ))}
      </tbody>
    </table>
  );
}

// ── Sub-components ─────────────────────────────────────────────────────

function Th({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <th
      className={`px-3 py-2 text-[10px] font-medium uppercase tracking-[0.18em] text-slate-500 ${className ?? ""}`}
    >
      {children}
    </th>
  );
}

function Row({
  event,
  labels,
  locale,
  now,
}: {
  event: LiquidityEventResponse;
  labels: LiquidityTableLabels;
  locale: FormatLocale;
  now?: Date;
}) {
  const timeRelative = formatRelative(event.timestamp, locale, now);
  const timeAbsolute = formatAbsolute(event.timestamp);

  const kindLabel =
    event.liquidity_event_kind === "add" ? labels.kindAdd : labels.kindRemove;

  return (
    <tr className="border-b border-cosmos-700/30 transition-colors last:border-b-0 hover:bg-cosmos-700/20">
      <td className="px-3 py-2 text-slate-300" title={timeAbsolute ?? ""}>
        {timeRelative ?? "—"}
      </td>
      <td className="px-3 py-2">
        <KindBadge kind={event.liquidity_event_kind} label={kindLabel} />
      </td>
      <td className="px-3 py-2 text-right font-mono text-xs text-slate-200">
        {formatU64(event.amount_a)}
      </td>
      <td className="px-3 py-2 text-right font-mono text-xs text-slate-200">
        {formatU64(event.amount_b)}
      </td>
      <td
        className="px-3 py-2 font-mono text-xs text-slate-400"
        title={event.owner}
      >
        {shortenPubkey(event.owner)}
      </td>
      <td
        className="px-3 py-2 text-right font-mono text-xs text-slate-500"
        title={event.signature}
      >
        {shortenPubkey(event.signature)}
      </td>
    </tr>
  );
}

function KindBadge({
  kind,
  label,
}: {
  kind: LiquidityEventResponse["liquidity_event_kind"];
  label: string;
}) {
  const palette =
    kind === "add"
      ? "border-signal-good/40 bg-signal-good/10 text-signal-good"
      : "border-signal-bad/40 bg-signal-bad/10 text-signal-bad";
  return (
    <span
      className={`inline-flex items-center rounded-full border px-2 py-0.5 font-mono text-[10px] uppercase tracking-widest ${palette}`}
    >
      {label}
    </span>
  );
}

function formatU64(value: number): string {
  return Intl.NumberFormat().format(value);
}