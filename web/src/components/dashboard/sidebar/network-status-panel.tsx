"use client";

/**
 * The Solana block at the foot of the sidebar. `/api/network` (slot, latency)
 * and `/api/ingestion` (the dot) are polled apart: a slow or failed half never
 * holds back the other, and a failed half reads "offline", never a silent "—".
 */

import { useEffect, useRef, useState } from "react";
import { useTranslations } from "next-intl";

import { ApiClientError } from "@/lib/api/errors";
import { fetchIngestionBrowser } from "@/lib/api/browser/ingestion";
import { fetchNetworkBrowser } from "@/lib/api/browser/network";
import type {
  FreshnessStatusResponse,
  IngestionResponse,
  NetworkResponse,
} from "@/lib/api/schema/zod.gen";

/** How often the panel re-fetches the network status. */
const POLL_INTERVAL_MS = 10_000;

type Half<T> = { phase: "loading" } | { phase: "ready"; data: T } | { phase: "error" };

const LOADING = { phase: "loading" } as const;

export function NetworkStatusPanel({
  collapsed = false,
}: {
  /** On the collapsed rail only the dot shows. ⚠️ CSS visibility, so the poll never restarts. */
  collapsed?: boolean;
}) {
  const t = useTranslations("Dashboard.Sidebar.network");

  const network = usePolled(fetchNetworkBrowser);
  const ingestion = usePolled(fetchIngestionBrowser);

  const { dotClass, labelClass, labelKey } = presentation(ingestion);
  const label = t(labelKey);
  const offline = t("offline");

  return (
    <>
      <div
        className={`border border-dash-rule bg-dash-surface px-[14px] py-3 ${collapsed ? "lg:hidden" : ""}`}
      >
        <header className="mb-2 flex items-center justify-between">
          <span className="font-dash-mono text-[11px] font-medium tracking-[0.12em] text-dash-ink-3 uppercase">
            {t("title")}
          </span>
          <span
            className={`inline-flex items-center gap-2 text-[12px] ${labelClass}`}
          >
            <StatusDot className={dotClass} />
            {label}
          </span>
        </header>

        <dl className="flex flex-col gap-1.5 text-[12px]">
          <StatRow
            label={t("slot")}
            value={networkValue(network, (d) => d.slot, offline)}
            down={network.phase === "error"}
          />
          <StatRow
            label={t("latency")}
            value={networkValue(network, (d) => `${d.rpcLatencyMs} ms`, offline)}
            down={network.phase === "error"}
          />
        </dl>
      </div>

      {collapsed && (
        <div
          title={`${t("title")} — ${label}`}
          className="hidden justify-center border border-dash-rule bg-dash-surface py-3 lg:flex"
        >
          <StatusDot className={dotClass} />
          <span className="sr-only">{label}</span>
        </div>
      )}
    </>
  );
}

/** Fetch now, then every `POLL_INTERVAL_MS`; a tick is skipped while the last is in flight. */
function usePolled<T>(fetcher: () => Promise<T>): Half<T> {
  const [state, setState] = useState<Half<T>>(LOADING);
  const inFlight = useRef(false);

  useEffect(() => {
    const load = async () => {
      if (inFlight.current) return;
      inFlight.current = true;
      try {
        setState({ phase: "ready", data: await fetcher() });
      } catch (err) {
        if (!(err instanceof ApiClientError)) {
          console.error("NetworkStatusPanel: unexpected error", err);
        }
        setState({ phase: "error" });
      } finally {
        inFlight.current = false;
      }
    };
    const timer = setInterval(() => void load(), POLL_INTERVAL_MS);
    setTimeout(() => void load(), 0);
    return () => clearInterval(timer);
  }, [fetcher]);

  return state;
}

// The label always doubles the dot's colour.

type Presentation = { dotClass: string; labelClass: string; labelKey: string };

function presentation(state: Half<IngestionResponse>): Presentation {
  if (state.phase === "loading") {
    return {
      dotClass: "bg-dash-ink-3",
      labelClass: "text-dash-ink-2",
      labelKey: "connecting",
    };
  }
  if (state.phase === "error") {
    return {
      dotClass: "bg-dash-down",
      labelClass: "text-dash-down",
      labelKey: "offline",
    };
  }
  return BY_FRESHNESS[state.data.freshness];
}

const BY_FRESHNESS: Record<FreshnessStatusResponse, Presentation> = {
  live: {
    dotClass: "bg-dash-accent",
    labelClass: "text-dash-ink-2",
    labelKey: "live",
  },
  delayed: {
    dotClass: "bg-dash-warn",
    labelClass: "text-dash-warn",
    labelKey: "delayed",
  },
  stale: {
    dotClass: "bg-dash-down",
    labelClass: "text-dash-down",
    labelKey: "stale",
  },
};

function networkValue(
  state: Half<NetworkResponse>,
  pick: (data: NetworkResponse) => string,
  offline: string,
): string {
  if (state.phase === "ready") return pick(state.data);
  return state.phase === "error" ? offline : "—";
}

function StatusDot({ className }: { className: string }) {
  return <span aria-hidden="true" className={`h-2 w-2 shrink-0 ${className}`} />;
}

function StatRow({ label, value, down }: { label: string; value: string; down: boolean }) {
  return (
    <div className="flex items-baseline justify-between gap-2">
      <dt className="text-dash-ink-3">{label}</dt>
      <dd className={`font-dash-mono tabular-nums ${down ? "text-dash-down" : "text-dash-ink"}`}>
        {value}
      </dd>
    </div>
  );
}
