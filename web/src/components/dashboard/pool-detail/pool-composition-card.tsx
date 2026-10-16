/**
 * The pool composition donut and legend, for a non-null `composition`: both sides valued.
 */

import { getTranslations } from "next-intl/server";

import { InfoPopover } from "@/components/shared/info-popover";
import type { EmbeddedTokenResponse } from "@/lib/api/schema/zod.gen";
import type { PoolComposition } from "@/lib/format/pool-composition";
import { shareToCircleCoords } from "@/lib/format/pool-composition";
import { formatTokenAmount } from "@/lib/format/format-token-amount";
import { formatUsdCompact } from "@/lib/format/format-usd";

// The viewBox is centred on (0,0), radius 1, scaled by the <svg>'s width.
const RADIUS_OUTER = 1;
const RADIUS_INNER = 0.62;

const COLOR_A = "#8b5cf6"; // sothoth violet
const COLOR_B = "#3b82f6"; // signal blue

const CARD_CLASS =
  "flex flex-col rounded-[8px] border border-sothoth-500/15 bg-cosmos-700/50 px-3 py-2 lg:px-4 lg:py-3";

const LABEL_CLASS =
  "text-[17px] font-semibold tracking-[0.2em] text-slate-400 uppercase";

export async function PoolCompositionCard({
  label,
  tokenA,
  tokenB,
  composition,
  reserveA,
  reserveB,
  className,
  info,
}: {
  label: string;
  tokenA: EmbeddedTokenResponse;
  tokenB: EmbeddedTokenResponse;
  composition: PoolComposition;
  /** Reserve in native units (integer string from the API) per side. */
  reserveA: string;
  reserveB: string;
  /** Extra classes from the parent (e.g. `h-full` to match a sibling block). */
  className?: string;
  /** Definition of the card, shown in an ⓘ popover next to the label. */
  info?: string;
}) {
  const tShell = info ? await getTranslations("Dashboard.shell") : null;

  return (
    <div className={`${CARD_CLASS} ${className ?? ""}`}>
      <div className="flex items-center gap-1.5">
        <p className={LABEL_CLASS}>{label}</p>
        {info && tShell && (
          <InfoPopover label={tShell("metricInfo")} iconSize={14}>
            {info}
          </InfoPopover>
        )}
      </div>

      {/* `flex-1` keeps the donut centred when the card stretches to the KPI block. */}
      <div className="mt-1 flex flex-1 items-center gap-5">
        <Donut
          shareA={composition.shareA}
          tvlUsd={composition.valueAUsd + composition.valueBUsd}
        />

        <div className="flex flex-1 flex-col gap-3 text-[14px]">
          <LegendRow
            color={COLOR_A}
            symbol={tokenA.symbol ?? "?"}
            share={composition.shareA}
            valueUsd={composition.valueAUsd}
            reserve={reserveA}
            decimals={tokenA.decimals}
          />
          <LegendRow
            color={COLOR_B}
            symbol={tokenB.symbol ?? "?"}
            share={composition.shareB}
            valueUsd={composition.valueBUsd}
            reserve={reserveB}
            decimals={tokenB.decimals}
          />
        </div>
      </div>
    </div>
  );
}

function Donut({
  shareA,
  tvlUsd,
}: {
  shareA: number;
  tvlUsd: number;
}) {
  // ⚠️ A full-circle arc collapses to zero length: one side at 100% is drawn as a ring.
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
            <circle cx="0" cy="0" r={RADIUS_INNER} fill="#0c0a14" />
          </>
        )}
      </svg>

      <div className="absolute inset-0 flex flex-col items-center justify-center text-center">
        <span className="font-display text-[14px] font-bold tracking-[0.02em] text-[#f5f2ff] lg:text-[15px]">
          {formatUsdCompact(String(tvlUsd))}
        </span>
        <span className="mt-0.5 text-[10px] font-semibold tracking-[0.2em] text-slate-400 uppercase">
          TVL
        </span>
      </div>
    </div>
  );
}

/** An arc covering `share` of the circle from `startShare`, clockwise from 12 o'clock. */
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

/** One side: symbol and USD value, then the reserve and its share of the TVL. */
function LegendRow({
  color,
  symbol,
  share,
  valueUsd,
  reserve,
  decimals,
}: {
  color: string;
  symbol: string;
  share: number;
  valueUsd: number;
  reserve: string;
  decimals: number;
}) {
  return (
    <div className="flex flex-col gap-0.5">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2 min-w-0">
          <span
            aria-hidden="true"
            className="h-2.5 w-2.5 shrink-0 rounded-full"
            style={{ backgroundColor: color }}
          />
          <span className="truncate font-medium text-slate-100">{symbol}</span>
        </div>
        <span className="font-mono text-slate-100">
          {formatUsdCompact(String(valueUsd))}
        </span>
      </div>
      <div className="flex items-center justify-between gap-3 pl-[18px] text-[12px] text-slate-400">
        <span className="truncate font-mono">
          {formatTokenAmount(reserve, decimals, symbol)}
        </span>
        <span className="font-mono">{(share * 100).toFixed(1)}%</span>
      </div>
    </div>
  );
}