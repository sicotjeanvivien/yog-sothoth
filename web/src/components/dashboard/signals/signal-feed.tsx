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
 * One full-width row card per signal — the feed reads top-down like
 * an alert log, values aligned on the right column. Severity is
 * carried by shape *and* color (icon + left accent bar + tinted
 * background + colored value), never by hue alone. The severity icon
 * is a left column spanning the card's height; next to it, three
 * lines:
 *
 *   1. token pair (→ pool; short address while unresolved) ·
 *      protocol · the metric value, large and severity-colored
 *   2. human summary phrased from the structured value (not the
 *      detector's raw English `message`) · current USD prices
 *   3. relative time · raw detector tag · crossed threshold
 *
 * A detector this component doesn't know yet falls back to the raw
 * `message` (or value/threshold pair): the feed must render whatever
 * the engine grows next, just less prettily.
 */

import type { FC } from "react";

import { useLocale, useTranslations } from "next-intl";

import { Link } from "@/i18n/navigation";
import type { SignalResponse, Severity } from "@/lib/api/schema/signal";
import type { TokenResponse } from "@/lib/api/schema/token";
import { formatPercent, formatSignedPercent } from "@/lib/format/format-percent";
import { formatPrice } from "@/lib/format/pool-price";
import { formatProtocolLabel } from "@/lib/format/format-protocol";
import { formatRelativeTime } from "@/lib/format/format-relative-time";
import { formatShortAddress } from "@/lib/format/format-short-address";

import {
  AlertOctagonIcon,
  AlertTriangleIcon,
  InfoIcon,
  type IconProps,
} from "@/components/shared/icon";
import { PoolPairCell } from "../pools/pool-pair-cell";
import { useSignalStream, type StreamStatus } from "./use-signal-stream";

// ── Severity styling ──────────────────────────────────────────────────

// Row tint per severity: left accent bar + border + background, one
// clear rung apart (critical > warning > info). Info stays close to
// neutral — when most of the feed is warning/critical, a tinted info
// row would flatten the scale.
const SEVERITY_CARD: Record<Severity, string> = {
  info: "border-sothoth-500/15 border-l-sky-400/60 bg-cosmos-700/40",
  warning: "border-amber-400/30 border-l-amber-400 bg-amber-400/[0.06]",
  critical: "border-rose-400/40 border-l-rose-400 bg-rose-500/[0.10]",
};

// Icon and headline value inherit the severity color.
const SEVERITY_COLOR: Record<Severity, string> = {
  info: "text-sky-300",
  warning: "text-amber-300",
  critical: "text-rose-300",
};

// Two distinct shapes across the escalation (triangle → octagon), so
// the scale survives color-blindness; the label stays in `title` +
// sr-only text.
const SEVERITY_ICON: Record<Severity, FC<IconProps>> = {
  info: InfoIcon,
  warning: AlertTriangleIcon,
  critical: AlertOctagonIcon,
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

/**
 * Human summary of a card, phrased from the structured `value` rather
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
 * side qualifies, and the card omits the block.
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
        <ul className="flex flex-col gap-2">
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
  const pairResolved =
    signal.tokenA.symbol !== null && signal.tokenB.symbol !== null;
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
  const severityLabel = t(`severity.${signal.severity}`);
  const SeverityIcon = SEVERITY_ICON[signal.severity];

  return (
    <li
      className={`flex items-center gap-4 rounded-[8px] border border-l-4 px-4 py-3 ${SEVERITY_CARD[signal.severity]}`}
    >
      {/* Severity icon — a full-height left column, the first thing
          the eye meets when scanning the feed. Below it, a terse
          category tag so the detector kind reads at a glance without
          parsing the summary; the color stays with severity, the tag
          stays neutral. */}
      <span
        title={severityLabel}
        className={`flex w-[44px] flex-col items-center gap-1 ${SEVERITY_COLOR[signal.severity]}`}
      >
        <SeverityIcon size={32} />
        <span className="sr-only">{severityLabel}</span>
        {known && (
          <span className="text-[10px] font-semibold tracking-[0.08em] text-slate-400 uppercase">
            {t(`detectors.${signal.detector}.tag`)}
          </span>
        )}
      </span>

      <div className="flex min-w-0 flex-1 flex-col gap-1.5">
        {/* Line 1 — pair · protocol · value */}
        <div className="flex items-center gap-3">
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
          <span className="whitespace-nowrap text-[12px] text-slate-500">
            {formatProtocolLabel(signal.protocol)}
          </span>
          <span
            className={`ml-auto truncate font-mono text-[20px] font-semibold ${SEVERITY_COLOR[signal.severity]}`}
          >
            {value}
          </span>
        </div>

        {/* Line 2 — summary · prices */}
        <div className="flex flex-wrap items-baseline gap-x-3 gap-y-0.5 text-[13px]">
          <span className="min-w-0 flex-1 leading-[1.5] text-slate-300">
            {detectorSummary(signal, t, locale)}
          </span>
          {prices && (
            <span className="whitespace-nowrap text-[12px] text-slate-500">
              {prices}
            </span>
          )}
        </div>

        {/* Line 3 — time · detector tag · threshold */}
        <div className="flex items-center justify-between gap-2 text-[12px] text-slate-400">
          <time
            dateTime={signal.triggeredAt}
            className="whitespace-nowrap text-slate-500"
          >
            {formatRelativeTime(signal.triggeredAt, locale)}
          </time>
          <span className="flex min-w-0 items-center gap-2">
            <span className="truncate font-mono text-[11px] text-slate-500">
              {signal.detector}
            </span>
            <span aria-hidden className="text-slate-600">
              ·
            </span>
            <span className="whitespace-nowrap">
              {t("detail.threshold")}{" "}
              <span className="font-mono text-slate-200">{threshold}</span>
            </span>
          </span>
        </div>
      </div>
    </li>
  );
}
