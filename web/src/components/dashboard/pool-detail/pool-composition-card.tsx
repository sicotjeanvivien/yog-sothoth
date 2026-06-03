/**
 * Pool composition card — donut chart + legend.
 *
 * Renders only when both sides of the pool have a known price (the
 * computation helper returns null otherwise). The parent block
 * decides what to show in the null case; this component assumes a
 * non-null `composition` and a non-null `tvlUsd`.
 *
 * The donut is a single SVG with two arc paths:
 *   - arc A from 12 o'clock clockwise to share-A boundary
 *   - arc B from share-A boundary clockwise back to 12 o'clock
 *
 * Center text shows the TVL value with a small "TVL" label below.
 *
 * Colours come from the same palette as the rest of the dashboard
 * (sothoth-500 for token A, a contrasting blue for token B). They
 * are passed inline so the SVG renders identically in light/dark
 * preview tooling without depending on Tailwind variable
 * resolution inside `<svg>`.
 */

import type { TokenResponse } from "@/lib/api/schema/token";
import type { PoolComposition } from "@/lib/format/pool-composition";
import { shareToCircleCoords } from "@/lib/format/pool-composition";
import { formatUsdCompact } from "@/lib/format/format-usd";

// Visual tuning. The donut viewBox is centered on (0,0) with the
// circle of radius 1, then scaled by the wrapping <svg>'s width.
const RADIUS_OUTER = 1;
const RADIUS_INNER = 0.62;

// Token A — Yog-Scope brand violet. Token B — a complementary
// blue. Stay readable on the dark cosmos background.
const COLOR_A = "#8b5cf6"; // sothoth violet
const COLOR_B = "#3b82f6"; // signal blue

const CARD_CLASS =
  "rounded-[8px] border border-sothoth-500/15 bg-cosmos-900/40 px-5 py-4 lg:px-6 lg:py-5";

const LABEL_CLASS =
  "text-[11px] font-semibold tracking-[0.2em] text-slate-400 uppercase";

export function PoolCompositionCard({
  label,
  tokenA,
  tokenB,
  composition,
  tvlUsd,
}: {
  label: string;
  tokenA: TokenResponse;
  tokenB: TokenResponse;
  composition: PoolComposition;
  tvlUsd: string | null;
}) {
  console.log(tokenA);
  console.log(tokenB);
  console.log(composition);
  
  
  return (
    <div className={CARD_CLASS}>
      <p className={LABEL_CLASS}>{label}</p>

      <div className="mt-3 flex items-center gap-5">
        {/* Donut */}
        <Donut shareA={composition.shareA} tvlUsd={tvlUsd} />

        {/* Legend */}
        <div className="flex flex-1 flex-col gap-2 text-[13px]">
          <LegendRow
            color={COLOR_A}
            symbol={tokenA.symbol ?? "?"}
            share={composition.shareA}
          />
          <LegendRow
            color={COLOR_B}
            symbol={tokenB.symbol ?? "?"}
            share={composition.shareB}
          />
        </div>
      </div>
    </div>
  );
}

// ── Sub-components ───────────────────────────────────────────────────

function Donut({
  shareA,
  tvlUsd,
}: {
  shareA: number;
  tvlUsd: string | null;
}) {
  // Edge case: when one side is 100%, a single full-circle arc
  // doesn't render correctly with the path-based approach (the
  // arc collapses to zero length). We special-case by rendering a
  // single ring of the dominant colour.
  const isSingleColor = shareA >= 0.999 || shareA <= 0.001;
  const dominantColor = shareA >= 0.5 ? COLOR_A : COLOR_B;

  return (
    <div className="relative h-[120px] w-[120px] shrink-0">
      <svg
        viewBox="-1.1 -1.1 2.2 2.2"
        xmlns="http://www.w3.org/2000/svg"
        className="h-full w-full"
      >
        {isSingleColor ? (
          // Single-colour ring (one side at 100%)
          <g>
            <circle
              cx="0"
              cy="0"
              r={RADIUS_OUTER}
              fill={dominantColor}
            />
            <circle cx="0" cy="0" r={RADIUS_INNER} fill="#0c0a14" />
          </g>
        ) : (
          <>
            <ArcPath share={shareA} startShare={0} color={COLOR_A} />
            <ArcPath
              share={1 - shareA}
              startShare={shareA}
              color={COLOR_B}
            />
            {/* Punch out the centre to make it a donut */}
            <circle cx="0" cy="0" r={RADIUS_INNER} fill="#0c0a14" />
          </>
        )}
      </svg>

      {/* Centre label */}
      <div className="absolute inset-0 flex flex-col items-center justify-center text-center">
        <span className="font-display text-[14px] font-bold tracking-[0.02em] text-[#f5f2ff] lg:text-[15px]">
          {formatUsdCompact(tvlUsd)}
        </span>
        <span className="mt-0.5 text-[9px] font-semibold tracking-[0.2em] text-slate-400 uppercase">
          TVL
        </span>
      </div>
    </div>
  );
}

/**
 * Render a single coloured arc that starts at `startShare` of the
 * way around the circle (measured from 12 o'clock, clockwise) and
 * covers `share` of the full circumference.
 *
 * The path is built as: line from centre → arc to first point →
 * arc along the outer circle to the second point → close back to
 * centre. The donut shape comes from the centre circle drawn on
 * top by the parent.
 */
function ArcPath({
  share,
  startShare,
  color,
}: {
  share: number;
  startShare: number;
  color: string;
}) {
  const start = shareToCircleCoords(startShare);
  const end = shareToCircleCoords(startShare + share);
  const largeArc = share > 0.5 ? 1 : 0;

  const d = [
    `M 0 0`,
    `L ${start.x} ${start.y}`,
    `A ${RADIUS_OUTER} ${RADIUS_OUTER} 0 ${largeArc} 1 ${end.x} ${end.y}`,
    "Z",
  ].join(" ");

  return <path d={d} fill={color} />;
}

function LegendRow({
  color,
  symbol,
  share,
}: {
  color: string;
  symbol: string;
  share: number;
}) {
  return (
    <div className="flex items-center justify-between gap-3">
      <div className="flex items-center gap-2 min-w-0">
        <span
          aria-hidden="true"
          className="h-2.5 w-2.5 shrink-0 rounded-full"
          style={{ backgroundColor: color }}
        />
        <span className="truncate font-medium text-slate-100">{symbol}</span>
      </div>
      <span className="font-mono text-slate-400">
        {(share * 100).toFixed(1)}%
      </span>
    </div>
  );
}