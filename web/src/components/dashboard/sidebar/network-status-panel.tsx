"use client";

/**
 * Network status panel — the "Solana Live" block at the foot of the
 * sidebar.
 *
 * Autonomous: it owns its own data lifecycle. It fetches the BFF
 * `/api/network/status` route on mount and then polls every
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
import type { Freshness, NetworkStatusResponse } from "@/lib/api/schema/network-status";

/** How often the panel re-fetches the network status. */
const POLL_INTERVAL_MS = 10_000;

// ── Panel state ───────────────────────────────────────────────────────

/**
 * The panel's view state — a discriminated union so the render
 * branches exhaustively.
 */
type PanelState =
  | { phase: "loading" }
  | { phase: "ready"; data: NetworkStatusResponse }
  | { phase: "error" };

// ── Component ─────────────────────────────────────────────────────────

export function NetworkStatusPanel() {
  const t = useTranslations("Dashboard.Sidebar.network");

  const [state, setState] = useState<PanelState>({ phase: "loading" });

  // Guards against overlapping requests: if a fetch is still in
  // flight when the next interval tick fires, that tick is skipped.
  const inFlight = useRef(false);

  const load = useCallback(async () => {
    if (inFlight.current) return;
    inFlight.current = true;
    try {
      const response = await fetch("/api/network/status");
      if (!response.ok) {
        // The BFF already mapped upstream failures to an HTTP error;
        // any non-2xx here means "we can't show live data".
        setState({ phase: "error" });
        return;
      }
      const data = (await response.json()) as NetworkStatusResponse;
      setState({ phase: "ready", data });
    } catch {
      // Network error reaching our own BFF.
      setState({ phase: "error" });
    } finally {
      inFlight.current = false;
    }
  }, []);

  // Fetch once on mount, then poll. The interval is cleared on
  // unmount so a navigated-away panel stops polling.
  useEffect(() => {
    const timer = setInterval(() => void load(), POLL_INTERVAL_MS);
    // Appel initial asynchrone
    setTimeout(() => void load(), 0);
    return () => clearInterval(timer);
  }, [load]);

  return (
    <div className="mt-5 rounded-[4px] border border-sothoth-500/15 bg-cosmos-800/65 p-[14px]">
      <header className="flex items-center justify-between">
        <div className="flex items-center gap-[7px]">
          <SolanaGlyph size={20} />
          <span className="text-[11px] font-semibold tracking-[0.04em] text-slate-300">
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
  );
}

// ── Value formatting ──────────────────────────────────────────────────
//
// The placeholder dash is shown in every non-ready state — both while
// loading and on error. The error case is signalled by the badge, not
// by the values, so the rows stay calm.

/** Slot value for the current state, or "—" when not ready. */
function slotValue(state: PanelState): string {
  return state.phase === "ready" ? state.data.slot : "—";
}

/** Latency value for the current state, or "—" when not ready. */
function latencyValue(state: PanelState): string {
  return state.phase === "ready" ? `${state.data.rpcLatencyMs} ms` : "—";
}

// ── Status badge ──────────────────────────────────────────────────────

/**
 * The top-right badge: a coloured dot plus a short label.
 *
 *   - loading : neutral dot, "connecting" label;
 *   - error   : red dot, "offline" label;
 *   - ready   : dot + label driven by the freshness verdict.
 */
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

/**
 * The ready-state badge — colour and label depend on the freshness
 * verdict. Only the `live` verdict gets the pulsing dot; `delayed`
 * and `stale` use a steady dot, since a pulsing "stale" would read
 * as healthy.
 */
function FreshnessBadge({ freshness }: { freshness: Freshness }) {
  const t = useTranslations("Dashboard.Sidebar.network");

  // Per-verdict presentation. `signal-good` / `signal-warn` /
  // `signal-bad` are the shared status colours used across the
  // dashboard.
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

/**
 * Badge shell — a status dot followed by an uppercase label. The dot
 * pulses only when `pulse` is set (live verdict).
 */
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
      className={`flex items-center gap-[5px] text-[9px] font-semibold tracking-[0.18em] uppercase ${labelClass}`}
    >
      <StatusDot colorClass={dotClass} pulse={pulse} />
      {children}
    </span>
  );
}

/**
 * Status dot. When `pulse` is set, a ping-animated clone sits under a
 * static dot (same effect as the prototype's `LiveDot`). Otherwise a
 * single steady dot.
 */
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

// ── Stat row ──────────────────────────────────────────────────────────

/** A label/value row in the panel's definition list. */
function StatRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline justify-between">
      <dt className="text-[10.5px] text-slate-500">{label}</dt>
      <dd className="font-mono text-[11px] text-slate-300">{value}</dd>
    </div>
  );
}