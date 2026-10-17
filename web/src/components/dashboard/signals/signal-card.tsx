"use client";

/**
 * The canonical card of a signal, for the `/signals` feed and the Overview (`compact`).
 * ⚠️ Severity is shape and colour, never hue alone. An unknown detector falls back to its raw
 * `message`: the card renders whatever the engine grows next.
 */

import { useLocale, useTranslations } from "next-intl";

import type { SeverityResponse, SignalResponse } from "@/lib/api/schema/zod.gen";
import {
  formatPercent,
  formatSignedPercent,
} from "@/lib/format/format-percent";
import { formatProtocolLabel } from "@/lib/format/format-protocol";
import { formatRelativeTime } from "@/lib/format/format-relative-time";
import { formatShortAddress } from "@/lib/format/format-short-address";

import { PoolPairCell } from "./pool-pair-cell";
import {
  KNOWN_DETECTORS,
  SEVERITY_COLOR,
  SEVERITY_ICON,
} from "./signal-display";

// Info stays near neutral: in a mostly warning/critical feed, a tinted info row flattens the scale.
const SEVERITY_CARD: Record<SeverityResponse, string> = {
  info: "border-sothoth-500/15 border-l-sky-400/60 bg-cosmos-700/40",
  warning: "border-amber-400/30 border-l-amber-400 bg-amber-400/[0.06]",
  critical: "border-rose-400/40 border-l-rose-400 bg-rose-500/[0.10]",
};

type Translate = ReturnType<typeof useTranslations>;

/** The summary, phrased from the structured `value`, not the detector's English `message`. */
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
      // Positive: A→B dominates, the flow pours into token B.
      const toward = signal.value.startsWith("-")
        ? signal.tokenA.symbol
        : signal.tokenB.symbol;
      const percent = formatPercent(signal.value, locale);
      return toward
        ? t("detectors.flow_imbalance.summaryToward", {
            percent,
            token: toward,
          })
        : t("detectors.flow_imbalance.summary", { percent });
    }
    case "tvl_drain":
      return t("detectors.tvl_drain.summary", {
        percent: formatPercent(signal.value, locale),
      });
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

export function SignalCard({
  signal,
  compact = false,
}: {
  signal: SignalResponse;
  compact?: boolean;
}) {
  const t = useTranslations("Dashboard.Signals.feed");
  const locale = useLocale();

  const known = KNOWN_DETECTORS.has(signal.detector);
  // The drain ratio is unsigned: "+61%" would read as growth.
  const value = !known
    ? signal.value
    : signal.detector === "tvl_drain"
      ? formatPercent(signal.value, locale)
      : formatSignedPercent(signal.value, locale);
  const threshold =
    signal.threshold === null
      ? "—"
      : known
        ? formatPercent(signal.threshold, locale)
        : signal.threshold;
  const severityLabel = t(`severity.${signal.severity}`);
  const SeverityIcon = SEVERITY_ICON[signal.severity];

  const severityColumn = (
    <span
      title={severityLabel}
      className={`flex w-[44px] flex-col items-center gap-1 ${SEVERITY_COLOR[signal.severity]}`}
    >
      <SeverityIcon size={32} />
      <span className="sr-only">{severityLabel}</span>
      {known && (
        <span className="text-[11px] font-semibold tracking-[0.08em] text-slate-400 uppercase">
          {t(`detectors.${signal.detector}.tag`)}
        </span>
      )}
    </span>
  );

  const valueFigure = (
    <span
      className={`ml-auto truncate font-mono text-[24px] font-semibold ${SEVERITY_COLOR[signal.severity]}`}
    >
      {value}
    </span>
  );

  if (compact) {
    return (
      <li
        className={`flex items-center gap-4 rounded-[8px] border border-l-4 px-4 py-3 ${SEVERITY_CARD[signal.severity]}`}
      >
        {severityColumn}
        <div className="flex min-w-0 flex-col gap-0.5">
          <PairLabel signal={signal} />
          {/* Relative to now: the SSR text may lag the client by a minute. */}
          <time
            dateTime={signal.triggeredAt}
            className="text-[13px] whitespace-nowrap text-slate-500"
            suppressHydrationWarning
          >
            {formatRelativeTime(signal.triggeredAt, locale)}
          </time>
        </div>
        {valueFigure}
      </li>
    );
  }

  return (
    <li
      className={`flex items-center gap-4 rounded-[8px] border border-l-4 px-4 py-4 ${SEVERITY_CARD[signal.severity]}`}
    >
      {severityColumn}

      <div className="flex min-w-0 flex-1 flex-col gap-2">
        <div className="flex items-center gap-3">
          <PairLabel signal={signal} />
          <span className="whitespace-nowrap text-[13px] text-slate-500">
            {formatProtocolLabel(signal.protocol)}
          </span>
          {valueFigure}
        </div>

        <div className="flex flex-wrap items-baseline gap-x-3 gap-y-0.5 text-[14px]">
          <span className="min-w-0 flex-1 leading-[1.5] text-slate-300">
            {detectorSummary(signal, t, locale)}
          </span>
        </div>

        <div className="flex items-center justify-between gap-2 text-[13px] text-slate-400">
          {/* Relative to now: the SSR text may lag the client by a minute. */}
          <time
            dateTime={signal.triggeredAt}
            className="whitespace-nowrap text-slate-500"
            suppressHydrationWarning
          >
            {formatRelativeTime(signal.triggeredAt, locale)}
          </time>
          <span className="flex min-w-0 items-center gap-2">
            <span className="truncate font-mono text-[12px] text-slate-500">
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

/** The pair, or the short pool address while a symbol is unresolved. */
function PairLabel({ signal }: { signal: SignalResponse }) {
  const pairResolved =
    signal.tokenA.symbol !== null && signal.tokenB.symbol !== null;

  return (
    <div className="min-w-0">
      {pairResolved ? (
        <PoolPairCell tokenA={signal.tokenA} tokenB={signal.tokenB} />
      ) : (
        <span className="font-mono text-[14px] text-sothoth-200">
          {formatShortAddress(signal.poolAddress)}
        </span>
      )}
    </div>
  );
}
