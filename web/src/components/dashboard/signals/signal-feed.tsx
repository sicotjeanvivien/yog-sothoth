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
 * One card per signal: severity badge · relative time · token pair
 * (linking to the pool, falling back to the short address while the
 * pair is unresolved) · a human-readable headline per detector,
 * phrased from the structured value — not the detector's raw English
 * `message` · a detail footer with the exact value, the crossed
 * threshold and the raw detector tag for traceability.
 *
 * A detector this component doesn't know yet falls back to the raw
 * `message` (or value/threshold pair): the feed must render whatever
 * the engine grows next, just less prettily.
 */

import { useLocale, useTranslations } from "next-intl";

import { Link } from "@/i18n/navigation";
import type { SignalResponse, Severity } from "@/lib/api/schema/signal";
import { formatPercent, formatSignedPercent } from "@/lib/format/format-percent";
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

  return (
    <li className="flex flex-col gap-3 rounded-[8px] border border-sothoth-500/15 bg-cosmos-700/40 p-4">
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
        <Link
          href={`/pools/${signal.poolAddress}`}
          className="group inline-block underline-offset-4 hover:underline"
        >
          {pairResolved ? (
            <PoolPairCell tokenA={signal.tokenA} tokenB={signal.tokenB} />
          ) : (
            <span className="font-mono text-[14px] text-sothoth-200">
              {formatShortAddress(signal.poolAddress)}
            </span>
          )}
        </Link>
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

      <div className="mt-auto flex items-center justify-between gap-2 border-t border-sothoth-500/10 pt-3 text-[12px] text-slate-400">
        <span>
          {t("detail.value")}{" "}
          <span className="font-mono text-slate-200">{value}</span>
          <span className="mx-1.5 text-slate-600">·</span>
          {t("detail.threshold")}{" "}
          <span className="font-mono text-slate-200">{threshold}</span>
        </span>
        <span className="truncate font-mono text-[11px] text-slate-500">
          {signal.detector}
        </span>
      </div>
    </li>
  );
}
