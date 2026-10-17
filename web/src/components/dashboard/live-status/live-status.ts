/**
 * The four states of the "Live" indicator, from the polled `/api/ingestion`: its three
 * freshness values, and "offline" when the call fails. The label always doubles the colour.
 */

import type {
  FreshnessStatusResponse,
  IngestionResponse,
} from "@/lib/api/schema/zod.gen";

import type { Half } from "./use-polled";

export type LiveState = FreshnessStatusResponse | "offline" | "connecting";

export type LiveStatus = {
  state: LiveState;
  dotClass: string;
  textClass: string;
  /** Key under `Dashboard.liveStatus.states`. */
  labelKey: LiveState;
};

const STATUS: Record<LiveState, LiveStatus> = {
  live: {
    state: "live",
    dotClass: "bg-dash-up",
    textClass: "text-dash-up",
    labelKey: "live",
  },
  delayed: {
    state: "delayed",
    dotClass: "bg-dash-warn",
    textClass: "text-dash-warn",
    labelKey: "delayed",
  },
  stale: {
    state: "stale",
    dotClass: "bg-dash-ink-3",
    textClass: "text-dash-ink-3",
    labelKey: "stale",
  },
  offline: {
    state: "offline",
    dotClass: "bg-dash-down",
    textClass: "text-dash-down",
    labelKey: "offline",
  },
  connecting: {
    state: "connecting",
    dotClass: "bg-dash-ink-3",
    textClass: "text-dash-ink-2",
    labelKey: "connecting",
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
