/**
 * Shared display vocabulary for signals — severity → icon/color, and
 * the set of detectors the UI knows how to phrase and format.
 *
 * Lives outside `signal-feed.tsx` (a `"use client"` module) so Server
 * Components (the Overview's latest-signals block) can share the same
 * mapping without crossing the client boundary.
 */

import type { FC } from "react";

import type { Severity } from "@/lib/api/schema/signal";
import {
  AlertOctagonIcon,
  AlertTriangleIcon,
  InfoIcon,
  type IconProps,
} from "@/components/shared/icon";

/** Icon and headline value inherit the severity color. */
export const SEVERITY_COLOR: Record<Severity, string> = {
  info: "text-sky-300",
  warning: "text-amber-300",
  critical: "text-rose-300",
};

/**
 * Two distinct shapes across the escalation (triangle → octagon), so
 * the scale survives color-blindness; the label stays in `title` +
 * sr-only text wherever these are rendered.
 */
export const SEVERITY_ICON: Record<Severity, FC<IconProps>> = {
  info: InfoIcon,
  warning: AlertTriangleIcon,
  critical: AlertOctagonIcon,
};

/**
 * Detectors the UI knows how to phrase (and whose value/threshold are
 * ratios to format as percents). An unknown detector still renders —
 * raw value, raw message — just less prettily.
 */
export const KNOWN_DETECTORS = new Set([
  "price_oracle_deviation",
  "flow_imbalance",
]);
