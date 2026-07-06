"use client";

/**
 * Local filter chips of the signal feed — severity × detector.
 *
 * Toggle chips with live counts (computed on the loaded list, so they
 * update as the SSE stream feeds it). Multi-select within a dimension,
 * AND across dimensions; nothing active = nothing hidden (the empty
 * state of an alert feed must never filter silently) — the selection
 * model lives in `lib/signals/filter-signals.ts`.
 *
 * Severity chips are the fixed closed set, colored like the cards.
 * Detector chips are DERIVED from the signals actually present:
 * labelled by their category tag when known (Prix/Flux), by the raw
 * detector name otherwise — a new detector shows up in the filters
 * with no code change, consistent with the cards' fallback.
 */

import { useTranslations } from "next-intl";

import type { Severity, SignalResponse } from "@/lib/api/schema/signal";

import { KNOWN_DETECTORS } from "./signal-display";

// Active chip styling per severity — same hues as the cards.
const SEVERITY_CHIP_ACTIVE: Record<Severity, string> = {
  info: "border-sky-400/40 bg-sky-400/10 text-sky-300",
  warning: "border-amber-400/40 bg-amber-400/10 text-amber-300",
  critical: "border-rose-400/40 bg-rose-400/10 text-rose-300",
};

const DETECTOR_CHIP_ACTIVE =
  "border-sothoth-500/40 bg-sothoth-600/20 text-sothoth-200";

const CHIP_INACTIVE =
  "border-sothoth-500/15 text-slate-400 hover:border-sothoth-500/30 hover:text-slate-300";

const SEVERITIES: readonly Severity[] = ["critical", "warning", "info"];

type Translate = ReturnType<typeof useTranslations>;

/**
 * Distinct detectors of the loaded list, known ones first (in their
 * canonical order), unknown ones after, alphabetically — a stable
 * chip order however the feed is sorted.
 */
function presentDetectors(signals: readonly SignalResponse[]): string[] {
  const present = new Set(signals.map((s) => s.detector));
  const known = [...KNOWN_DETECTORS].filter((d) => present.has(d));
  const unknown = [...present].filter((d) => !KNOWN_DETECTORS.has(d)).sort();
  return [...known, ...unknown];
}

function detectorLabel(detector: string, t: Translate): string {
  return KNOWN_DETECTORS.has(detector)
    ? t(`detectors.${detector}.tag`)
    : detector;
}

export function SignalFilters({
  signals,
  activeSeverities,
  activeDetectors,
  onToggleSeverity,
  onToggleDetector,
}: {
  signals: readonly SignalResponse[];
  activeSeverities: ReadonlySet<Severity>;
  activeDetectors: ReadonlySet<string>;
  onToggleSeverity: (severity: Severity) => void;
  onToggleDetector: (detector: string) => void;
}) {
  const t = useTranslations("Dashboard.Signals.feed");

  const severityCounts = new Map<Severity, number>();
  const detectorCounts = new Map<string, number>();
  for (const signal of signals) {
    severityCounts.set(
      signal.severity,
      (severityCounts.get(signal.severity) ?? 0) + 1,
    );
    detectorCounts.set(
      signal.detector,
      (detectorCounts.get(signal.detector) ?? 0) + 1,
    );
  }

  return (
    <div className="mb-4 flex flex-wrap items-center gap-x-5 gap-y-2">
      <FilterGroup label={t("filters.severity")}>
        {SEVERITIES.map((severity) => (
          <Chip
            key={severity}
            label={t(`severity.${severity}`)}
            count={severityCounts.get(severity) ?? 0}
            active={activeSeverities.has(severity)}
            activeClass={SEVERITY_CHIP_ACTIVE[severity]}
            onToggle={() => onToggleSeverity(severity)}
          />
        ))}
      </FilterGroup>

      <FilterGroup label={t("filters.detector")}>
        {presentDetectors(signals).map((detector) => (
          <Chip
            key={detector}
            label={detectorLabel(detector, t)}
            count={detectorCounts.get(detector) ?? 0}
            active={activeDetectors.has(detector)}
            activeClass={DETECTOR_CHIP_ACTIVE}
            onToggle={() => onToggleDetector(detector)}
          />
        ))}
      </FilterGroup>
    </div>
  );
}

function FilterGroup({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="text-[12px] font-semibold tracking-[0.08em] text-slate-500 uppercase">
        {label}
      </span>
      {children}
    </div>
  );
}

function Chip({
  label,
  count,
  active,
  activeClass,
  onToggle,
}: {
  label: string;
  count: number;
  active: boolean;
  activeClass: string;
  onToggle: () => void;
}) {
  return (
    <button
      type="button"
      aria-pressed={active}
      onClick={onToggle}
      className={`inline-flex items-center gap-1.5 rounded-full border px-3 py-1 text-[13px] font-medium transition-colors ${
        active ? activeClass : CHIP_INACTIVE
      }`}
    >
      {label}
      <span
        className={`font-mono text-[12px] ${active ? "opacity-80" : "text-slate-600"}`}
      >
        {count}
      </span>
    </button>
  );
}
