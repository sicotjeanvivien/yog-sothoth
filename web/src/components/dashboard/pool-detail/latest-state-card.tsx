/**
 * Card showing the projected current state of a pool — latest reserves,
 * last observed sqrt_price (from the last swap), liquidity (from the
 * last liquidity event), and the last event identity itself.
 *
 * Two render branches:
 *   - With state data → `LatestStateCard`.
 *   - Without (404 from yog-api, meaning no swap or liquidity event
 *     has been observed for this pool yet) → `LatestStateEmpty`.
 *
 * Pure presentational; the parent decides which branch to render based
 * on the API outcome.
 */

import type { PoolCurrentStateResponse } from "@/lib/api/schema/pool-current-state-response";
import { formatAbsolute, formatRelative, type FormatLocale } from "@/lib/format/date";
import { shortenPubkey } from "@/lib/format/pubkey";

export type LatestStateLabels = {
  sectionTitle: string;
  reserveA: string;
  reserveB: string;
  lastEvent: string;
  lastEventKindSwap: string;
  lastEventKindLiquidityAdd: string;
  lastEventKindLiquidityRemove: string;
  sqrtPrice: string;
  liquidity: string;
  signature: string;
  updatedAt: string;
  notObservedYet: string;
};

type LatestStateCardProps = {
  state: PoolCurrentStateResponse;
  labels: LatestStateLabels;
  locale: FormatLocale;
  now?: Date;
};

export function LatestStateCard({ state, labels, locale, now }: LatestStateCardProps) {
  const lastEventRelative = formatRelative(state.last_event_at, locale, now);
  const updatedAtRelative = formatRelative(state.updated_at, locale, now);

  return (
    <section className="rounded-lg border border-cosmos-700/60 bg-cosmos-900/60 px-6 py-6 shadow-[0_0_40px_-12px_rgba(124,58,237,0.25)] backdrop-blur-sm">
      <header className="flex items-baseline justify-between gap-4">
        <h2 className="font-display text-lg tracking-wider text-sothoth-400">
          {labels.sectionTitle}
        </h2>
        <span
          className="text-[10px] uppercase tracking-[0.18em] text-slate-500"
          title={formatAbsolute(state.updated_at) ?? ""}
        >
          {labels.updatedAt}: {updatedAtRelative ?? "—"}
        </span>
      </header>

      <dl className="mt-6 grid grid-cols-2 gap-x-8 gap-y-4 sm:grid-cols-4">
        <MetaItem label={labels.reserveA}>
          <span className="font-mono text-sm text-slate-200">
            {formatU64(state.reserve_a)}
          </span>
        </MetaItem>

        <MetaItem label={labels.reserveB}>
          <span className="font-mono text-sm text-slate-200">
            {formatU64(state.reserve_b)}
          </span>
        </MetaItem>

        <MetaItem label={labels.sqrtPrice}>
          <span className="font-mono text-xs text-slate-300">
            {state.last_sqrt_price ?? "—"}
          </span>
        </MetaItem>

        <MetaItem label={labels.liquidity}>
          <span className="font-mono text-xs text-slate-300">
            {state.liquidity ?? "—"}
          </span>
        </MetaItem>
      </dl>

      <footer className="mt-6 flex flex-wrap items-baseline gap-x-4 gap-y-2 border-t border-cosmos-700/40 pt-4 text-xs text-slate-400">
        <span className="text-[10px] uppercase tracking-[0.18em] text-slate-500">
          {labels.lastEvent}
        </span>
        <LastEventKindBadge kind={state.last_event_kind} labels={labels} />
        <span title={formatAbsolute(state.last_event_at) ?? ""}>
          {lastEventRelative ?? "—"}
        </span>
        <span
          className="font-mono text-[11px] text-slate-500"
          title={state.last_signature}
        >
          {labels.signature}: {shortenPubkey(state.last_signature)}
        </span>
      </footer>
    </section>
  );
}

// ── Empty state — pool exists but no swap/liquidity event yet ──────────

type LatestStateEmptyProps = {
  title: string;
  description: string;
};

export function LatestStateEmpty({ title, description }: LatestStateEmptyProps) {
  return (
    <section className="rounded-lg border border-cosmos-700/40 bg-cosmos-900/40 px-6 py-10 text-center shadow-[0_0_40px_-16px_rgba(124,58,237,0.15)]">
      <h2 className="font-display text-lg tracking-wider text-slate-400">{title}</h2>
      <p className="mx-auto mt-2 max-w-md text-sm text-slate-500">{description}</p>
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
      <dd className="mt-1">{children}</dd>
    </div>
  );
}

function LastEventKindBadge({
  kind,
  labels,
}: {
  kind: PoolCurrentStateResponse["last_event_kind"];
  labels: LatestStateLabels;
}) {
  const palette = badgePalette(kind);
  const label =
    kind === "swap"
      ? labels.lastEventKindSwap
      : kind === "liquidity_add"
        ? labels.lastEventKindLiquidityAdd
        : labels.lastEventKindLiquidityRemove;
  return (
    <span
      className={`inline-flex items-center rounded-full border px-2 py-0.5 font-mono text-[10px] uppercase tracking-widest ${palette}`}
    >
      {label}
    </span>
  );
}

function badgePalette(
  kind: PoolCurrentStateResponse["last_event_kind"],
): string {
  switch (kind) {
    case "swap":
      return "border-sothoth-600/40 bg-sothoth-600/10 text-sothoth-400";
    case "liquidity_add":
      return "border-signal-good/40 bg-signal-good/10 text-signal-good";
    case "liquidity_remove":
      return "border-signal-bad/40 bg-signal-bad/10 text-signal-bad";
  }
}

/**
 * Format a u64 as a grouped decimal string. Server-side: relies on
 * the default Intl number formatter which uses thin spaces in fr,
 * commas in en — desired behaviour for both locales.
 *
 * u64 fits in JS number for values up to 2^53 (≈ 9 PB of SPL atomic
 * units), which covers every realistic pool reserve.
 */
function formatU64(value: number): string {
  return Intl.NumberFormat().format(value);
}