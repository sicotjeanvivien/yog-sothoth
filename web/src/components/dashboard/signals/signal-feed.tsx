"use client";

/**
 * Live signal feed — the visible end of the Signal Engine loop.
 *
 * Client Component: seeded with the server-rendered first page, then
 * kept live by `useSignalStream` (SSE, direct to the public gateway).
 * New signals prepend in place; the connection state is surfaced as a
 * small status dot so a broken stream is never mistaken for a quiet
 * one.
 *
 * One card per signal, tinted by severity: severity badge · relative
 * time · token pair (linking to the pool, falling back to the short
 * address while the pair is unresolved) with the metric value as the
 * severity-colored headline figure · a human-readable summary per
 * detector, phrased from the structured value — not the detector's
 * raw English `message` · a footer with the sides' current USD
 * prices, the crossed threshold and the raw detector tag.
 *
 * A detector this component doesn't know yet falls back to the raw
 * `message` (or value/threshold pair): the feed must render whatever
 * the engine grows next, just less prettily.
 */

import { useLocale, useTranslations } from "next-intl";

import { Link } from "@/i18n/navigation";
import type { SignalResponse, Severity } from "@/lib/api/schema/signal";
import type { TokenResponse } from "@/lib/api/schema/token";
import { formatPercent, formatSignedPercent } from "@/lib/format/format-percent";
import { formatPrice } from "@/lib/format/pool-price";
import { formatProtocolLabel } from "@/lib/format/format-protocol";
import { formatRelativeTime } from "@/lib/format/format-relative-time";
import { formatShortAddress } from "@/lib/format/format-short-address";

import { PoolPairCell } from "../pools/pool-pair-cell";
import { useSignalStream, type StreamStatus } from "./use-signal-stream";

// ── Severity badge ────────────────────────────────────────────────────

const SEVERITY_BADGE: Record<Severity, string> = {
  info: "border-sky-400/30 bg-sky-400/10 text-sky-300",
  warning: "border-amber-400/30 bg-amber-400/10 text-amber-300",
  critical: "border-rose-400/30 bg-rose-400/10 text-rose-300",
};

// Card tint per severity — same hues as the badge, at low opacity so a
// grid of 50 warnings doesn't turn into a wall of amber. Info stays on
// the neutral card: if everything is tinted, nothing stands out.
const SEVERITY_CARD: Record<Severity, string> = {
  info: "border-sothoth-500/15 bg-cosmos-700/40",
  warning: "border-amber-400/25 bg-amber-400/[0.04]",
  critical: "border-rose-400/30 bg-rose-500/[0.06]",
};

// The headline value inherits the severity color.
const SEVERITY_VALUE: Record<Severity, string> = {
  info: "text-sky-300",
  warning: "text-amber-300",
  critical: "text-rose-300",
};

// ── Status dot ────────────────────────────────────────────────────────

const STATUS_DOT: Record<StreamStatus, string> = {
  connecting: "bg-slate-500",
  live: "bg-emerald-400",
  reconnecting: "bg-amber-400",
};

// ── Per-detector wording ──────────────────────────────────────────────

/** Detectors this feed knows how to phrase (and format as percents). */
const KNOWN_DETECTORS = new Set(["price_oracle_deviation", "flow_imbalance"]);

type Translate = ReturnType<typeof useTranslations>;

/** Human title of a card — the detector, in words. */
function detectorTitle(signal: SignalResponse, t: Translate): string {
  if (!KNOWN_DETECTORS.has(signal.detector)) {
    return signal.detector;
  }
  return t(`detectors.${signal.detector}.title`);
}

/**
 * Human headline of a card, phrased from the structured `value` rather
 * than the detector's raw English `message`.
 */
function detectorSummary(
  signal: SignalResponse,
  t: Translate,
  locale: string,
): string {
  switch (signal.detector) {
    case "price_oracle_deviation":
      return t("detectors.price_oracle_deviation.summary", {
        deviation: formatSignedPercent(signal.value, locale),
      });
    case "flow_imbalance": {
      // Sign convention: value > 0 = A→B volume dominates, i.e. the
      // flow pours *into* token B; negative pours into token A.
      const toward = signal.value.startsWith("-")
        ? signal.tokenA.symbol
        : signal.tokenB.symbol;
      const percent = formatPercent(signal.value, locale);
      return toward
        ? t("detectors.flow_imbalance.summaryToward", { percent, token: toward })
        : t("detectors.flow_imbalance.summary", { percent });
    }
    default:
      return (
        signal.message ??
        t("fallback", {
          value: signal.value,
          threshold: signal.threshold ?? "—",
        })
      );
  }
}

/**
 * "birry $0.000007758 · USDC $1.00" — the *current* USD price of each
 * resolved side (latest oracle fetch, not the price at trigger time).
 * Sides without a symbol or a price are skipped; `null` when neither
 * side qualifies, and the card omits the line.
 */
function tokenPriceLine(signal: SignalResponse): string | null {
  const side = (token: TokenResponse): string | null =>
    token.symbol && token.price
      ? `${token.symbol} $${formatPrice(Number(token.price.usd))}`
      : null;

  const parts = [side(signal.tokenA), side(signal.tokenB)].filter(
    (p): p is string => p !== null,
  );
  return parts.length > 0 ? parts.join(" · ") : null;
}

// ── Component ─────────────────────────────────────────────────────────

export function SignalFeed({
  initial,
}: {
  initial: readonly SignalResponse[];
}) {
  const t = useTranslations("Dashboard.Signals.feed");
  const locale = useLocale();
  const { signals, status } = useSignalStream(initial);

  return (
    <section className="mt-2 px-6 pb-10 lg:px-10">
      <div className="mb-4 flex items-center justify-between">
        <h2 className="text-[12px] font-semibold tracking-[0.28em] text-slate-400 uppercase">
          {t("title")}
        </h2>
        <span className="flex items-center gap-2 text-[12px] text-slate-400">
          <span
            className={`h-2 w-2 rounded-full ${STATUS_DOT[status]}`}
            aria-hidden
          />
          {t(`status.${status}`)}
        </span>
      </div>

      {signals.length === 0 ? (
        <p className="rounded-[8px] border border-sothoth-500/15 bg-cosmos-700/40 px-4 py-6 text-[14px] text-slate-400">
          {t("empty")}
        </p>
      ) : (
        <ul className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          {signals.map((signal) => (
            <SignalCard key={signal.id} signal={signal} t={t} locale={locale} />
          ))}
        </ul>
      )}
    </section>
  );
}

// ── Card ──────────────────────────────────────────────────────────────

function SignalCard({
  signal,
  t,
  locale,
}: {
  signal: SignalResponse;
  t: Translate;
  locale: string;
}) {
  const pairResolved = signal.tokenA.symbol !== null && signal.tokenB.symbol !== null;
  const known = KNOWN_DETECTORS.has(signal.detector);
  const value = known
    ? formatSignedPercent(signal.value, locale)
    : signal.value;
  const threshold =
    signal.threshold === null
      ? "—"
      : known
        ? formatPercent(signal.threshold, locale)
        : signal.threshold;
  const prices = tokenPriceLine(signal);

  return (
    <li
      className={`flex flex-col gap-3 rounded-[8px] border p-4 ${SEVERITY_CARD[signal.severity]}`}
    >
      <div className="flex items-center justify-between">
        <span
          className={`inline-flex rounded-full border px-2 py-0.5 text-[11px] font-semibold tracking-wide uppercase ${SEVERITY_BADGE[signal.severity]}`}
        >
          {t(`severity.${signal.severity}`)}
        </span>
        <time
          dateTime={signal.triggeredAt}
          className="text-[12px] whitespace-nowrap text-slate-500"
        >
          {formatRelativeTime(signal.triggeredAt, locale)}
        </time>
      </div>

      <div>
        <div className="flex items-center justify-between gap-3">
          <Link
            href={`/pools/${signal.poolAddress}`}
            className="group inline-block min-w-0 underline-offset-4 hover:underline"
          >
            {pairResolved ? (
              <PoolPairCell tokenA={signal.tokenA} tokenB={signal.tokenB} />
            ) : (
              <span className="font-mono text-[14px] text-sothoth-200">
                {formatShortAddress(signal.poolAddress)}
              </span>
            )}
          </Link>
          <span
            className={`truncate font-mono text-[20px] font-semibold ${SEVERITY_VALUE[signal.severity]}`}
          >
            {value}
          </span>
        </div>
        <p className="mt-1 text-[12px] text-slate-500">
          {formatProtocolLabel(signal.protocol)}
        </p>
      </div>

      <div>
        <p className="text-[14px] font-semibold text-slate-100">
          {detectorTitle(signal, t)}
        </p>
        <p className="mt-1 text-[13px] leading-[1.5] text-slate-300">
          {detectorSummary(signal, t, locale)}
        </p>
      </div>

      <div className="mt-auto border-t border-sothoth-500/10 pt-3 text-[12px] text-slate-400">
        {prices && <p className="mb-1 truncate text-slate-500">{prices}</p>}
        <div className="flex items-center justify-between gap-2">
          <span>
            {t("detail.threshold")}{" "}
            <span className="font-mono text-slate-200">{threshold}</span>
          </span>
          <span className="truncate font-mono text-[11px] text-slate-500">
            {signal.detector}
          </span>
        </div>
      </div>
    </li>
  );
}
