"use client";

/**
 * Network status panel — the "Solana Live" block at the foot of the
 * sidebar.
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
 *               coloured green / orange / red for live / delayed /
 *               stale;
 *   - error   : the fetch failed — an explicit "offline" state
 *               (red dot + label), never a silent "—". An health
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

import { SolanaGlyph } from "@/components/shared/icon";
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

  return (
    <>
      <div
        className={`mt-5 rounded-[4px] border border-sothoth-500/15 bg-cosmos-800/65 p-[14px] ${collapsed ? "lg:hidden" : ""}`}
      >
        <header className="flex items-center justify-between">
          <div className="flex items-center gap-[7px]">
            <SolanaGlyph size={20} />
            <span className="text-[12px] font-semibold tracking-[0.04em] text-slate-300">
              {t("title")}
            </span>
          </div>
          <StatusBadge state={state} />
        </header>

        <dl className="mt-3 flex flex-col gap-[7px]">
          <StatRow label={t("slot")} value={slotValue(state)} />
          <StatRow label={t("latency")} value={latencyValue(state)} />
        </dl>
      </div>

      {collapsed && (
        <div
          title={`${t("title")} — ${statusLabel(state, t)}`}
          className="mt-5 hidden justify-center rounded-[4px] border border-sothoth-500/15 bg-cosmos-800/65 py-3 lg:flex"
        >
          <StatusDot
            colorClass={statusDotClass(state)}
            pulse={state.phase === "ready" && state.data.freshness === "live"}
          />
          <span className="sr-only">{statusLabel(state, t)}</span>
        </div>
      )}
    </>
  );
}

// ── Collapsed-dot state mapping ───────────────────────────────────────

type TranslateNetwork = ReturnType<typeof useTranslations>;

function statusDotClass(state: PanelState): string {
  if (state.phase === "loading") return "bg-slate-500";
  if (state.phase === "error") return "bg-signal-bad";
  return {
    live: "bg-signal-good",
    delayed: "bg-signal-warn",
    stale: "bg-signal-bad",
  }[state.data.freshness];
}

function statusLabel(state: PanelState, t: TranslateNetwork): string {
  if (state.phase === "loading") return t("connecting");
  if (state.phase === "error") return t("offline");
  return t(state.data.freshness);
}

// ── Value formatting ──────────────────────────────────────────────────
// (unchanged below — copy from the previous version)

function slotValue(state: PanelState): string {
  return state.phase === "ready" ? state.data.slot : "—";
}

function latencyValue(state: PanelState): string {
  return state.phase === "ready" ? `${state.data.rpcLatencyMs} ms` : "—";
}

function StatusBadge({ state }: { state: PanelState }) {
  const t = useTranslations("Dashboard.Sidebar.network");

  if (state.phase === "loading") {
    return (
      <Badge dotClass="bg-slate-500" labelClass="text-slate-500">
        {t("connecting")}
      </Badge>
    );
  }

  if (state.phase === "error") {
    return (
      <Badge dotClass="bg-signal-bad" labelClass="text-signal-bad">
        {t("offline")}
      </Badge>
    );
  }

  return <FreshnessBadge freshness={state.data.freshness} />;
}

function FreshnessBadge({ freshness }: { freshness: Freshness }) {
  const t = useTranslations("Dashboard.Sidebar.network");

  const presentation: Record<
    Freshness,
    { dotClass: string; labelClass: string; labelKey: string; pulse: boolean }
  > = {
    live: {
      dotClass: "bg-signal-good",
      labelClass: "text-signal-good",
      labelKey: "live",
      pulse: true,
    },
    delayed: {
      dotClass: "bg-signal-warn",
      labelClass: "text-signal-warn",
      labelKey: "delayed",
      pulse: false,
    },
    stale: {
      dotClass: "bg-signal-bad",
      labelClass: "text-signal-bad",
      labelKey: "stale",
      pulse: false,
    },
  };

  const { dotClass, labelClass, labelKey, pulse } = presentation[freshness];

  return (
    <Badge dotClass={dotClass} labelClass={labelClass} pulse={pulse}>
      {t(labelKey)}
    </Badge>
  );
}

function Badge({
  children,
  dotClass,
  labelClass,
  pulse = false,
}: {
  children: React.ReactNode;
  dotClass: string;
  labelClass: string;
  pulse?: boolean;
}) {
  return (
    <span
      className={`flex items-center gap-[5px] text-[10px] font-semibold tracking-[0.18em] uppercase ${labelClass}`}
    >
      <StatusDot colorClass={dotClass} pulse={pulse} />
      {children}
    </span>
  );
}

function StatusDot({ colorClass, pulse }: { colorClass: string; pulse: boolean }) {
  if (!pulse) {
    return <span className={`h-1.5 w-1.5 rounded-full ${colorClass}`} />;
  }
  return (
    <span className="relative h-1.5 w-1.5">
      <span className={`absolute inset-0 animate-ping rounded-full ${colorClass}`} />
      <span className={`absolute inset-0 rounded-full ${colorClass}`} />
    </span>
  );
}

function StatRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline justify-between">
      <dt className="text-[11px] text-slate-500">{label}</dt>
      <dd className="font-mono text-[12px] text-slate-300">{value}</dd>
    </div>
  );
}