/**
 * The signals' display vocabulary. ⚠️ Kept out of the `"use client"` `signal-feed.tsx`, so
 * Server Components share it.
 */

import type { FC } from "react";

import type { SeverityResponse, SignalResponse } from "@/lib/api/schema/zod.gen";
import { formatPercent, formatSignedPercent } from "@/lib/format/format-percent";
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

/**
 * A severity badge: filled to be looked at, outlined for information. ⚠️ Dark text on the
 * fills: white reads at 2.7:1 on them.
 */
export const SEVERITY_BADGE: Record<SeverityResponse, string> = {
  critical: "bg-dash-down text-dash-surface",
  warning: "bg-dash-warn text-dash-surface",
  info: "text-dash-ink-2 shadow-[inset_0_0_0_1px_var(--color-dash-ink-2)]",
};

/** Two shapes across the escalation, so the scale survives colour-blindness. */
export const SEVERITY_ICON: Record<SeverityResponse, FC<IconProps>> = {
  info: InfoIcon,
  warning: AlertTriangleIcon,
  critical: AlertOctagonIcon,
};

/** Detectors the UI phrases. */
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

/** How many signals the Overview's signals block shows. */
export const LATEST_SIGNALS_COUNT = 7;

export function formatSignalValue(signal: SignalResponse, locale: string): string {
  switch (signal.valueUnit) {
    case "raw":
      return signal.value;
    case "ratio":
      return formatPercent(signal.value, locale);
    case "signedRatio":
      return formatSignedPercent(signal.value, locale);
  }
}

export function formatSignalThreshold(signal: SignalResponse, locale: string): string {
  if (signal.threshold === null) {
    return "—";
  }
  return signal.valueUnit === "raw" ? signal.threshold : formatPercent(signal.threshold, locale);
}

export function leanedSymbolOf(signal: SignalResponse): string | null {
  return signal.leansToward === null ? null : signal[signal.leansToward].symbol;
}
