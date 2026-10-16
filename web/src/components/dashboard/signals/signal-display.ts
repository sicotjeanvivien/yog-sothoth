/**
 * The signals' display vocabulary. ⚠️ Apart from `signal-feed.tsx`, a `"use client"` module,
 * so Server Components share it without crossing the client boundary.
 */

import type { FC } from "react";

import type { SeverityResponse } from "@/lib/api/schema/zod.gen";
import {
  AlertOctagonIcon,
  AlertTriangleIcon,
  InfoIcon,
  type IconProps,
} from "@/components/shared/icon";

export const SEVERITY_COLOR: Record<SeverityResponse, string> = {
  info: "text-sky-300",
  warning: "text-amber-300",
  critical: "text-rose-300",
};

/** Two shapes across the escalation, so the scale survives colour-blindness. */
export const SEVERITY_ICON: Record<SeverityResponse, FC<IconProps>> = {
  info: InfoIcon,
  warning: AlertTriangleIcon,
  critical: AlertOctagonIcon,
};

/** Detectors the UI phrases, with ratios as percents; an unknown one renders raw. */
export const KNOWN_DETECTORS = new Set([
  "price_oracle_deviation",
  "flow_imbalance",
  "tvl_drain",
]);

/** Shared by every `useSignalStream` consumer: a broken stream must never look quiet. */
export const STATUS_DOT: Record<
  "connecting" | "live" | "reconnecting",
  string
> = {
  connecting: "bg-slate-500",
  live: "bg-emerald-400",
  reconnecting: "bg-amber-400",
};

/** How many signals the Overview's latest-signals block shows. */
export const LATEST_SIGNALS_COUNT = 5;
