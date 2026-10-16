"use client";

/**
 * The Solana block at the foot of the sidebar. Polls `/api/network` (slot,
 * latency) and `/api/ingestion` (the dot) together; each half fails alone, to
 * "—" or to an explicit "offline" dot. A tick is skipped while the previous
 * one is in flight.
 */

import { useCallback, useEffect, useRef, useState } from "react";
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

// ── Panel state ───────────────────────────────────────────────────────

type Half<T> = { phase: "loading" } | { phase: "ready"; data: T } | { phase: "error" };

const LOADING = { phase: "loading" } as const;

// ── Component ─────────────────────────────────────────────────────────

export function NetworkStatusPanel({
  collapsed = false,
}: {
  /**
   * lg+ collapsed rail: the panel reduces to its status dot — the
   * "is it alive" signal stays permanently visible, slot/latency
   * come back on expand. Both variants render from the same polled
   * state; visibility is pure CSS so the poll never restarts.
   */
  collapsed?: boolean;
}) {
  const t = useTranslations("Dashboard.Sidebar.network");

  const [network, setNetwork] = useState<Half<NetworkResponse>>(LOADING);
  const [ingestion, setIngestion] = useState<Half<IngestionResponse>>(LOADING);

  // Guards against overlapping requests: if a fetch is still in
  // flight when the next interval tick fires, that tick is skipped.
  const inFlight = useRef(false);

  const load = useCallback(async () => {
    if (inFlight.current) return;
    inFlight.current = true;
    try {
      const [net, ing] = await Promise.allSettled([
        fetchNetworkBrowser(),
        fetchIngestionBrowser(),
      ]);
      setNetwork(settled(net));
      setIngestion(settled(ing));
    } finally {
      inFlight.current = false;
    }
  }, []);

  // Fetch once on mount, then poll. The interval is cleared on
  // unmount so a navigated-away panel stops polling.
  useEffect(() => {
    const timer = setInterval(() => void load(), POLL_INTERVAL_MS);
    // Initial async call.
    setTimeout(() => void load(), 0);
    return () => clearInterval(timer);
  }, [load]);

  const { dotClass, labelClass, labelKey } = presentation(ingestion);
  const label = t(labelKey);

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
          <StatRow label={t("slot")} value={slotValue(network)} />
          <StatRow label={t("latency")} value={latencyValue(network)} />
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

/** Any `ApiClientError` is the half's "offline"; anything else is logged too. */
function settled<T>(result: PromiseSettledResult<T>): Half<T> {
  if (result.status === "fulfilled") return { phase: "ready", data: result.value };
  if (!(result.reason instanceof ApiClientError)) {
    console.error("NetworkStatusPanel: unexpected error", result.reason);
  }
  return { phase: "error" };
}

// ── State presentation ────────────────────────────────────────────────
//
// Violet means live (the accent marks the live state); the warning and
// negative roles take over when the data lags or the API is gone. The
// label always doubles the dot's colour.

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

// ── Value formatting ──────────────────────────────────────────────────

function slotValue(state: Half<NetworkResponse>): string {
  return state.phase === "ready" ? state.data.slot : "—";
}

function latencyValue(state: Half<NetworkResponse>): string {
  return state.phase === "ready" ? `${state.data.rpcLatencyMs} ms` : "—";
}

function StatusDot({ className }: { className: string }) {
  return <span aria-hidden="true" className={`h-2 w-2 shrink-0 ${className}`} />;
}

function StatRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline justify-between gap-2">
      <dt className="text-dash-ink-3">{label}</dt>
      <dd className="font-dash-mono text-dash-ink tabular-nums">{value}</dd>
    </div>
  );
}
