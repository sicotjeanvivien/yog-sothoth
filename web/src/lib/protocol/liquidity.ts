/** The liquidity tab's two charts, as data: the TVL curve on a tight axis, and its flows per bar. */

import { slotsOf, type Slot } from "@/lib/overview/chart";
import type { FrozenPeriod } from "@/lib/overview/period";

/** The curve's box, in the SVG's own units: the line keeps `CURVE_MARGIN` clear above and below. */
export const CURVE_BOX = { width: 1000, height: 140 } as const;

const CURVE_MARGIN = 8;

export type CurvePoint = { at: Date; value: number };

export type Curve = {
  points: CurvePoint[];
  /** `x` and `y` of each point, in [`CURVE_BOX`] units. */
  coords: { x: number; y: number }[];
  line: string;
  area: string;
  /** The axis' ends: the lowest and highest TVL, so a small move still shows. */
  low: number;
  high: number;
};

/** The TVL points, a point without a value left out; `null` under two points. */
export function curveOf(items: readonly { at: string; tvlUsd: string | null }[]): Curve | null {
  const points = items.flatMap((item) => {
    const value = item.tvlUsd === null ? NaN : Number.parseFloat(item.tvlUsd);
    return Number.isFinite(value) ? [{ at: new Date(item.at), value }] : [];
  });
  if (points.length < 2) {
    return null;
  }
  points.sort((a, b) => a.at.getTime() - b.at.getTime());
  const low = Math.min(...points.map((p) => p.value));
  const high = Math.max(...points.map((p) => p.value));
  const first = points[0]!.at.getTime();
  const span = points[points.length - 1]!.at.getTime() - first;
  const inner = CURVE_BOX.height - 2 * CURVE_MARGIN;
  const coords = points.map((p) => ({
    x: ((p.at.getTime() - first) / span) * CURVE_BOX.width,
    y:
      high === low
        ? CURVE_BOX.height / 2
        : CURVE_MARGIN + ((high - p.value) / (high - low)) * inner,
  }));
  const line = `M${coords.map((c) => `${round(c.x)} ${round(c.y)}`).join(" L")}`;
  const area = `${line} L${CURVE_BOX.width} ${CURVE_BOX.height} L0 ${CURVE_BOX.height} Z`;
  return { points, coords, line, area, low, high };
}

/** One bar of the flows: `null` where the liquidity was not measured whole. */
export type FlowSlot = {
  from: Date;
  to: Date;
  created: number | null;
  added: number | null;
  removed: number | null;
};

type LiquidityBar = { from: string; createdUsd: string; addedUsd: string; removedUsd: string };

/** The flows in the period's slots (`slotsOf`): a slot not whole was not measured. */
export function flowSlotsOf(
  period: FrozenPeriod,
  asOf: string,
  bars: readonly LiquidityBar[],
): FlowSlot[] {
  const slots = (pick: (bar: LiquidityBar) => string): Slot[] =>
    slotsOf(
      period,
      asOf,
      bars.map((bar) => ({ from: bar.from, value: pick(bar) })),
    );
  const created = slots((bar) => bar.createdUsd);
  const added = slots((bar) => bar.addedUsd);
  const removed = slots((bar) => bar.removedUsd);
  return added.map((slot, i) => ({
    from: slot.from,
    to: slot.to,
    created: created[i]?.value ?? null,
    added: slot.value,
    removed: removed[i]?.value ?? null,
  }));
}

function round(value: number): number {
  return Math.round(value * 10) / 10;
}
