"use client";

/**
 * The signal feed's filter chips, severity × detector, counted on the loaded list (the
 * selection model is `lib/signals/filter-signals.ts`). Detector chips derive from the signals
 * present: a new detector appears with no code change.
 */

import { useTranslations } from "next-intl";

import { InfoPopover } from "@/components/shared/info-popover";
import type { SeverityResponse, SignalResponse } from "@/lib/api/schema/zod.gen";

import { KNOWN_DETECTORS } from "./signal-display";

const SEVERITY_CHIP_ACTIVE: Record<SeverityResponse, string> = {
  info: "border-sky-400/40 bg-sky-400/10 text-sky-300",
  warning: "border-amber-400/40 bg-amber-400/10 text-amber-300",
  critical: "border-rose-400/40 bg-rose-400/10 text-rose-300",
};

const DETECTOR_CHIP_ACTIVE =
  "border-sothoth-500/40 bg-sothoth-600/20 text-sothoth-200";

const CHIP_INACTIVE =
  "border-sothoth-500/15 text-slate-400 hover:border-sothoth-500/30 hover:text-slate-300";

const SEVERITIES: readonly SeverityResponse[] = ["critical", "warning", "info"];

type Translate = ReturnType<typeof useTranslations>;

/** Known detectors first in canonical order, then the unknown alphabetically: a stable order. */
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
  activeSeverities: ReadonlySet<SeverityResponse>;
  activeDetectors: ReadonlySet<string>;
  onToggleSeverity: (severity: SeverityResponse) => void;
  onToggleDetector: (detector: string) => void;
}) {
  const t = useTranslations("Dashboard.Signals.feed");
  const tShell = useTranslations("Dashboard.shell");

  const severityCounts = new Map<SeverityResponse, number>();
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

      <FilterGroup
        label={t("filters.detector")}
        info={<DetectorExplanations signals={signals} t={t} tShell={tShell} />}
      >
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
  info,
}: {
  label: string;
  children: React.ReactNode;
  /** Optional ⓘ popover rendered right after the group label. */
  info?: React.ReactNode;
}) {
  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="flex items-center gap-1.5 text-[12px] font-semibold tracking-[0.08em] text-slate-500 uppercase">
        {label}
        {info}
      </span>
      {children}
    </div>
  );
}

/**
 * What each known detector present measures, in one ⓘ panel. No threshold in the copy: it is
 * env config, and each card shows the threshold crossed.
 */
function DetectorExplanations({
  signals,
  t,
  tShell,
}: {
  signals: readonly SignalResponse[];
  t: Translate;
  tShell: Translate;
}) {
  const knownPresent = presentDetectors(signals).filter((detector) =>
    KNOWN_DETECTORS.has(detector),
  );
  if (knownPresent.length === 0) {
    return null;
  }

  return (
    <InfoPopover label={tShell("metricInfo")} iconSize={14}>
      <div className="flex flex-col gap-2">
        {knownPresent.map((detector) => (
          <p key={detector}>
            <strong className="font-semibold text-slate-100">
              {t(`detectors.${detector}.tag`)}
            </strong>
            {" — "}
            {t(`detectors.${detector}.explain`)}
          </p>
        ))}
      </div>
    </InfoPopover>
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
