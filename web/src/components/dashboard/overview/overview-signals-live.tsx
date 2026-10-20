"use client";

/**
 * The Overview's latest signals, seeded by the server and kept live by `useSignalStream`:
 * severity, time, headline, and detector · protocol, with the stream-status dot.
 */

import type { ReactNode } from "react";
import { useLocale, useTranslations } from "next-intl";

import { SeverityBadge } from "@/components/dashboard/signals/severity-badge";
import {
  formatSignalValue,
  KNOWN_DETECTORS,
  LATEST_SIGNALS_COUNT,
  STATUS_DOT,
} from "@/components/dashboard/signals/signal-display";
import { useSignalStream } from "@/components/dashboard/signals/use-signal-stream";
import type { SignalResponse } from "@/lib/api/schema/zod.gen";
import { formatProtocolLabel } from "@/lib/format/format-protocol";
import { formatShortAddress } from "@/lib/format/format-short-address";

import { ROW_CLASS } from "./overview-layout";

const DAY_MS = 24 * 60 * 60 * 1000;

export function OverviewSignalsLive({
  initial,
  heading,
}: {
  initial: readonly SignalResponse[];
  heading: ReactNode;
}) {
  const t = useTranslations("Dashboard.Overview.signals");
  const tFeed = useTranslations("Dashboard.Signals.feed");
  const { signals, status } = useSignalStream(initial);
  const latest = signals.slice(0, LATEST_SIGNALS_COUNT);

  return (
    <>
      <div className="flex items-start justify-between gap-3">
        {heading}
        <span
          className={`mt-1.5 h-2 w-2 shrink-0 ${STATUS_DOT[status]}`}
          title={tFeed(`status.${status}`)}
        >
          <span className="sr-only">{tFeed(`status.${status}`)}</span>
        </span>
      </div>

      {latest.length === 0 ? (
        <p className="py-6 text-[14px] text-dash-ink-2">{t("empty")}</p>
      ) : (
        <div role="table" aria-label={t("tableLabel")}>
          {latest.map((signal) => (
            <SignalRow key={signal.id} signal={signal} />
          ))}
        </div>
      )}
    </>
  );
}

function SignalRow({ signal }: { signal: SignalResponse }) {
  const t = useTranslations("Dashboard.Overview.signals");
  const tFeed = useTranslations("Dashboard.Signals.feed");
  const locale = useLocale();

  // ⚠️ Isolated (FSI…PDI): a right-to-left symbol would otherwise reverse the whole line.
  const pair =
    signal.tokenA.symbol !== null && signal.tokenB.symbol !== null
      ? `\u2068${signal.tokenA.symbol}\u2069 / \u2068${signal.tokenB.symbol}\u2069`
      : formatShortAddress(signal.poolAddress);
  const value = formatSignalValue(signal, locale);
  const headline = KNOWN_DETECTORS.has(signal.detector)
    ? t(`headline.${signal.detector}`, { pair, value })
    : (signal.message ?? t("headline.fallback", { pair, value }));

  return (
    <div role="row" className={`${ROW_CLASS} grid-cols-[76px_52px_minmax(0,1fr)]`}>
      <div role="cell">
        <SeverityBadge
          severity={signal.severity}
          label={tFeed(`severity.${signal.severity}`)}
        />
      </div>
      <div role="cell" className="font-dash-mono text-[12px] text-dash-ink-2 tabular-nums">
        {/* Against now: the SSR text may differ from the client's near midnight. */}
        <time dateTime={signal.triggeredAt} suppressHydrationWarning>
          {formatTriggeredAt(signal.triggeredAt, locale)}
        </time>
      </div>
      <div role="cell" className="min-w-0">
        <div className="text-dash-ink">{headline}</div>
        <div className="mt-0.5 truncate font-dash-mono text-[11px] text-dash-ink-3">
          {signal.detector} · {formatProtocolLabel(signal.protocol)}
        </div>
      </div>
    </div>
  );
}

/** "14:48" within a day, "18 oct." beyond, in UTC like the charts. */
function formatTriggeredAt(iso: string, locale: string): string {
  const at = new Date(iso);
  const recent = Date.now() - at.getTime() < DAY_MS;
  return new Intl.DateTimeFormat(
    locale,
    recent
      ? { hour: "2-digit", minute: "2-digit", hourCycle: "h23", timeZone: "UTC" }
      : { day: "numeric", month: "short", timeZone: "UTC" },
  ).format(at);
}
