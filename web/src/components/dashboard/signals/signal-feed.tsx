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
 * Each row: severity badge · detector tag · pool link · message ·
 * relative time. The message is the detector's own human summary; rows
 * without one fall back to the raw value / threshold pair.
 */

import { useLocale, useTranslations } from "next-intl";

import { Link } from "@/i18n/navigation";
import type { SignalResponse, Severity } from "@/lib/api/schema/signal";
import { formatRelativeTime } from "@/lib/format/format-relative-time";
import { formatShortAddress } from "@/lib/format/format-short-address";

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
        <ul className="overflow-hidden rounded-[8px] border border-sothoth-500/15 bg-cosmos-700/40">
          {signals.map((signal) => (
            <li
              key={signal.id}
              className="flex flex-wrap items-center gap-x-4 gap-y-1 border-b border-sothoth-500/10 px-4 py-3 last:border-b-0"
            >
              <span
                className={`inline-flex rounded-full border px-2 py-0.5 text-[11px] font-semibold tracking-wide uppercase ${SEVERITY_BADGE[signal.severity]}`}
              >
                {t(`severity.${signal.severity}`)}
              </span>

              <span className="font-mono text-[12px] text-slate-400">
                {signal.detector}
              </span>

              <Link
                href={`/pools/${signal.poolAddress}`}
                className="font-mono text-[13px] text-sothoth-200 underline-offset-4 hover:underline"
              >
                {formatShortAddress(signal.poolAddress)}
              </Link>

              <span className="min-w-0 flex-1 truncate text-[14px] text-slate-300">
                {signal.message ??
                  t("fallback", {
                    value: signal.value,
                    threshold: signal.threshold ?? "—",
                  })}
              </span>

              <time
                dateTime={signal.triggeredAt}
                className="ml-auto text-[12px] whitespace-nowrap text-slate-500"
              >
                {formatRelativeTime(signal.triggeredAt, locale)}
              </time>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
