"use client";

/**
 * The page header's "Live · 2 s ago" button and its panel: state, last event, slot, RPC
 * latency, the page's price coverage, the four states' legend. ⚠️ Opens on click only.
 */

import {
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useLocale, useTranslations } from "next-intl";

import { ChevronDownIcon } from "@/components/shared/icon";
import { useDismiss } from "@/components/shared/use-dismiss";
import { fetchIngestionBrowser } from "@/lib/api/browser/ingestion";
import { fetchNetworkBrowser } from "@/lib/api/browser/network";
import type { NetworkResponse } from "@/lib/api/schema/zod.gen";
import { formatRelativeTime } from "@/lib/format/format-relative-time";

import { LIVE_LEGEND, liveStatusOf } from "./live-status";
import { usePolled } from "./use-polled";

export function LiveIndicator({
  coverage,
}: {
  /** The page's price coverage, streamed by the server; no row without it. */
  coverage?: ReactNode;
}) {
  const t = useTranslations("Dashboard.liveStatus");
  const ingestion = usePolled(fetchIngestionBrowser);

  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLSpanElement>(null);
  const panelId = useId();
  const close = useCallback(() => setOpen(false), []);
  useDismiss(rootRef, open, close);

  const status = liveStatusOf(ingestion);
  const label = t(`states.${status.state}`);
  const lastEventAt =
    ingestion.phase === "ready" ? ingestion.data.lastEventAt : null;

  return (
    <span ref={rootRef} className="relative inline-flex">
      <button
        type="button"
        aria-expanded={open}
        aria-controls={panelId}
        onClick={() => setOpen((value) => !value)}
        className={`inline-flex min-h-9 items-center gap-2 border px-[10px] text-[12px] transition-colors hover:border-dash-rule ${status.textClass} ${open ? "border-dash-rule" : "border-transparent"}`}
      >
        <Dot className={status.dotClass} />
        <span>
          {label}
          {lastEventAt && (
            <>
              {" · "}
              <Age iso={lastEventAt} />
            </>
          )}
        </span>
        <ChevronDownIcon
          size={12}
          className={`transition-transform ${open ? "rotate-180" : ""}`}
        />
      </button>

      {/* Rendered closed too, `hidden`: `aria-controls` must point at an element. */}
      <div
        id={panelId}
        role="group"
        aria-label={t("panelLabel")}
        hidden={!open}
        className="absolute top-full right-0 z-20 mt-1 w-[280px] border border-dash-rule bg-dash-surface-2 px-[14px] py-3 text-left text-[14px]"
      >
        {open && (
          <>
          <div className="mb-2 flex items-center justify-between">
            <span className="font-dash-mono text-[11px] font-medium tracking-[0.12em] text-dash-ink-3 uppercase">
              {t("network")}
            </span>
            <span
              className={`inline-flex items-center gap-2 text-[12px] ${status.textClass}`}
            >
              <Dot className={status.dotClass} />
              {label}
            </span>
          </div>

          <dl className="flex flex-col gap-1.5 text-[12px]">
            <Row
              label={t("lastEvent")}
              value={lastEventAt ? <Age iso={lastEventAt} /> : "—"}
            />
            <NetworkRows />
            {coverage !== undefined && (
              <Row label={t("priceCoverage")} value={coverage} />
            )}
          </dl>

          <ul className="mt-2.5 flex flex-wrap gap-x-3 gap-y-1.5 border-t border-dash-rule pt-2.5 text-[11px] text-dash-ink-2">
            {LIVE_LEGEND.map((entry) => (
              <li key={entry.state} className="inline-flex items-center gap-[5px]">
                <Dot className={entry.dotClass} />
                {t(`states.${entry.state}`)}
              </li>
            ))}
          </ul>
          </>
        )}
      </div>
    </span>
  );
}

/** Slot and RPC latency, polled only while the panel is open. */
function NetworkRows() {
  const t = useTranslations("Dashboard.liveStatus");
  const network = usePolled(fetchNetworkBrowser);
  const down = network.phase === "error";
  const value = (pick: (data: NetworkResponse) => string) =>
    network.phase === "ready"
      ? pick(network.data)
      : down
        ? t("states.offline")
        : "—";

  return (
    <>
      <Row label={t("slot")} value={value((data) => data.slot)} down={down} />
      <Row
        label={t("latency")}
        value={value((data) => `${data.rpcLatencyMs} ms`)}
        down={down}
      />
    </>
  );
}

/**
 * "2 s ago", ticking every second. ⚠️ Never in the future: a browser clock behind the server's
 * would otherwise read "in 3 s".
 */
function Age({ iso }: { iso: string }) {
  const locale = useLocale();
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const timer = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(timer);
  }, []);
  const at = new Date(Math.max(now.getTime(), Date.parse(iso)));
  return <>{formatRelativeTime(iso, locale, { now: at, style: "short" })}</>;
}

function Dot({ className }: { className: string }) {
  return <span aria-hidden="true" className={`h-2 w-2 shrink-0 ${className}`} />;
}

function Row({
  label,
  value,
  down = false,
}: {
  label: string;
  value: ReactNode;
  down?: boolean;
}) {
  return (
    <div className="flex items-baseline justify-between gap-2">
      <dt className="text-dash-ink-3">{label}</dt>
      <dd
        className={`font-dash-mono tabular-nums ${down ? "text-dash-down" : "text-dash-ink"}`}
      >
        {value}
      </dd>
    </div>
  );
}
