"use client";

/**
 * Network status panel — the Solana block at the foot of the sidebar:
 * state, slot, latency.
 *
 * Autonomous: it owns its own data lifecycle. It fetches yog-api
 * directly through the public gateway on mount and then polls every
 * `POLL_INTERVAL_MS`. The sidebar just mounts it — it knows nothing
 * about fetching or polling.
 *
 * # Three states
 *
 *   - loading : first fetch not yet returned — slot/latency show
 *               "—", the dot is neutral;
 *   - ready   : data in hand — slot, latency, and a freshness dot
 *               (see `presentation`);
 *   - error   : the fetch failed — an explicit "offline" state
 *               (negative dot + label), never a silent "—". An health
 *               panel that can't signal its own connection loss
 *               would defeat its purpose.
 *
 * # Polling
 *
 * `setInterval` at 10s, cleared on unmount. The first fetch fires
 * immediately so the panel doesn't sit empty for a full interval.
 * A slow request is not stacked on by the next tick — an in-flight
 * guard skips a tick while one is already running.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslations } from "next-intl";

import { ApiClientError } from "@/lib/api/errors";
import { fetchNetworkStatusBrowser } from "@/lib/api/browser/network-status";
import type {
  Freshness,
  NetworkStatusResponse,
} from "@/lib/api/schema/network-status";

/** How often the panel re-fetches the network status. */
const POLL_INTERVAL_MS = 10_000;

// ── Panel state ───────────────────────────────────────────────────────

type PanelState =
  | { phase: "loading" }
  | { phase: "ready"; data: NetworkStatusResponse }
  | { phase: "error" };

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

  const [state, setState] = useState<PanelState>({ phase: "loading" });

  // Guards against overlapping requests: if a fetch is still in
  // flight when the next interval tick fires, that tick is skipped.
  const inFlight = useRef(false);

  const load = useCallback(async () => {
    if (inFlight.current) return;
    inFlight.current = true;
    try {
      const data = await fetchNetworkStatusBrowser();
      setState({ phase: "ready", data });
    } catch (err) {
      // Any ApiClientError variant (timeout, network, http, validation)
      // collapses to the panel's "offline" state. The dot/label is
      // enough signal for this UI; we don't need to differentiate.
      // Errors that aren't ApiClientError should not reach here, but
      // if they do they get the same treatment — the panel is
      // best-effort by design.
      if (!(err instanceof ApiClientError)) {
        // Surface unexpected throws in the console for the developer;
        // the user still sees "offline".
        console.error("NetworkStatusPanel: unexpected error", err);
      }
      setState({ phase: "error" });
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

  const { dotClass, labelClass, labelKey } = presentation(state);
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
          <StatRow label={t("slot")} value={slotValue(state)} />
          <StatRow label={t("latency")} value={latencyValue(state)} />
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

// ── State presentation ────────────────────────────────────────────────
//
// Violet means live (the accent marks the live state); the warning and
// negative roles take over when the data lags or the API is gone. The
// label always doubles the dot's colour.

type Presentation = { dotClass: string; labelClass: string; labelKey: string };

function presentation(state: PanelState): Presentation {
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
  const byFreshness: Record<Freshness, Presentation> = {
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
  return byFreshness[state.data.freshness];
}

// ── Value formatting ──────────────────────────────────────────────────

function slotValue(state: PanelState): string {
  return state.phase === "ready" ? state.data.slot : "—";
}

function latencyValue(state: PanelState): string {
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
