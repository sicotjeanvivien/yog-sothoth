/**
 * Dense table listing swap events on a pool, ordered most-recent first.
 *
 * Pure presentational. Columns:
 *
 *   - Time (relative + absolute on hover)
 *   - Direction (A→B / B→A with arrow glyph)
 *   - Amount In / Amount Out (depending on direction)
 *   - Fee (sum of the four fee components, in the token bearing it)
 *   - Signature (short, linkable later to an explorer)
 *
 * The table avoids horizontal scrolling on a 1024+ wide layout by
 * keeping numeric columns right-aligned and reusing the same font-mono
 * styles as `pools-table.tsx`.
 */

import type { SwapEventResponse } from "@/lib/api/schema/swap-event-response";
import { formatAbsolute, formatRelative, type FormatLocale } from "@/lib/format/date";
import { shortenPubkey } from "@/lib/format/pubkey";

export type SwapsTableLabels = {
  time: string;
  direction: string;
  amountIn: string;
  amountOut: string;
  fee: string;
  signature: string;
  directionAtoB: string;
  directionBtoA: string;
};

type SwapsTableProps = {
  swaps: SwapEventResponse[];
  labels: SwapsTableLabels;
  locale: FormatLocale;
  now?: Date;
};

export function SwapsTable({ swaps, labels, locale, now }: SwapsTableProps) {
  return (
    <table className="w-full border-collapse text-left text-sm">
      <thead>
        <tr className="border-b border-cosmos-700/60">
          <Th>{labels.time}</Th>
          <Th>{labels.direction}</Th>
          <Th className="text-right">{labels.amountIn}</Th>
          <Th className="text-right">{labels.amountOut}</Th>
          <Th className="text-right">{labels.fee}</Th>
          <Th className="text-right">{labels.signature}</Th>
        </tr>
      </thead>
      <tbody>
        {swaps.map((swap) => (
          <Row
            key={swap.signature}
            swap={swap}
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
  swap,
  labels,
  locale,
  now,
}: {
  swap: SwapEventResponse;
  labels: SwapsTableLabels;
  locale: FormatLocale;
  now?: Date;
}) {
  const timeRelative = formatRelative(swap.timestamp, locale, now);
  const timeAbsolute = formatAbsolute(swap.timestamp);

  // Direction-relative amounts: by canonical convention the swap moves
  // tokens *from* one side *to* the other. For display, "In" is what
  // the trader sent, "Out" is what they received.
  const amountIn = swap.trade_direction === "a_to_b" ? swap.amount_a : swap.amount_b;
  const amountOut = swap.trade_direction === "a_to_b" ? swap.amount_b : swap.amount_a;

  const fee =
    swap.claiming_fee + swap.protocol_fee + swap.compounding_fee + swap.referral_fee;

  const directionLabel =
    swap.trade_direction === "a_to_b" ? labels.directionAtoB : labels.directionBtoA;

  return (
    <tr className="border-b border-cosmos-700/30 transition-colors last:border-b-0 hover:bg-cosmos-700/20">
      <td className="px-3 py-2 text-slate-300" title={timeAbsolute ?? ""}>
        {timeRelative ?? "—"}
      </td>
      <td className="px-3 py-2">
        <DirectionBadge direction={swap.trade_direction} label={directionLabel} />
      </td>
      <td className="px-3 py-2 text-right font-mono text-xs text-slate-200">
        {formatU64(amountIn)}
      </td>
      <td className="px-3 py-2 text-right font-mono text-xs text-slate-200">
        {formatU64(amountOut)}
      </td>
      <td className="px-3 py-2 text-right font-mono text-xs text-slate-400">
        {formatU64(fee)}
      </td>
      <td
        className="px-3 py-2 text-right font-mono text-xs text-slate-500"
        title={swap.signature}
      >
        {shortenPubkey(swap.signature)}
      </td>
    </tr>
  );
}

function DirectionBadge({
  direction,
  label,
}: {
  direction: SwapEventResponse["trade_direction"];
  label: string;
}) {
  const palette =
    direction === "a_to_b"
      ? "border-sothoth-600/40 bg-sothoth-600/10 text-sothoth-400"
      : "border-eldritch-600/40 bg-eldritch-600/10 text-eldritch-400";
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