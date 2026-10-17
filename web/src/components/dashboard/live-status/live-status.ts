/**
 * The four states of the "Live" indicator, from the polled `/api/ingestion`: its three
 * freshness values, and "offline" when the call fails. The label always doubles the colour.
 */

import type {
  FreshnessStatusResponse,
  IngestionResponse,
} from "@/lib/api/schema/zod.gen";

import type { Half } from "./use-polled";

/** Also the label's key under `Dashboard.liveStatus.states`. */
export type LiveState = FreshnessStatusResponse | "offline" | "connecting";

export type LiveStatus = {
  state: LiveState;
  dotClass: string;
  textClass: string;
};

const STATUS: Record<LiveState, LiveStatus> = {
  live: {
    state: "live",
    dotClass: "bg-dash-up",
    textClass: "text-dash-up",
  },
  delayed: {
    state: "delayed",
    dotClass: "bg-dash-warn",
    textClass: "text-dash-warn",
  },
  stale: {
    state: "stale",
    dotClass: "bg-dash-ink-3",
    textClass: "text-dash-ink-3",
  },
  offline: {
    state: "offline",
    dotClass: "bg-dash-down",
    textClass: "text-dash-down",
  },
  connecting: {
    state: "connecting",
    dotClass: "bg-dash-ink-3",
    textClass: "text-dash-ink-2",
  },
};

/** The legend's order, which is also the indicator's four states. */
export const LIVE_LEGEND: readonly LiveStatus[] = [
  STATUS.live,
  STATUS.delayed,
  STATUS.stale,
  STATUS.offline,
];

export function liveStatusOf(ingestion: Half<IngestionResponse>): LiveStatus {
  if (ingestion.phase === "loading") return STATUS.connecting;
  if (ingestion.phase === "error") return STATUS.offline;
  return STATUS[ingestion.data.freshness];
}
