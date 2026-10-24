/** The liquidity tab's two charts, as data: the TVL curve on a tight axis, and its flows per bar. */

import type { ProtocolResponse } from "@/lib/api/schema/zod.gen";
import { slotsOf, type Slot } from "@/lib/overview/chart";
import type { Period } from "@/lib/overview/period";

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

/** The protocol's TVL points, a point without a value left out; `null` under two points. */
export function curveOf(
  items: readonly { protocol: ProtocolResponse; at: string; tvlUsd: string | null }[],
  protocol: ProtocolResponse,
): Curve | null {
  const points = items.flatMap((item) => {
    const value = item.tvlUsd === null ? NaN : Number.parseFloat(item.tvlUsd);
    return item.protocol === protocol && Number.isFinite(value)
      ? [{ at: new Date(item.at), value }]
      : [];
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
export type FlowSlot = { from: Date; to: Date; added: number | null; removed: number | null };

type LiquidityBar = {
  protocol: ProtocolResponse;
  from: string;
  liquidity: { addedUsd: string; removedUsd: string } | null;
};

/** The protocol's flows in the period's slots: hours over 24 h, 24-hour slices beyond. */
export function flowSlotsOf(
  period: Period,
  asOf: string,
  bars: readonly LiquidityBar[],
  protocol: ProtocolResponse,
): FlowSlot[] {
  const mine = bars.filter((bar) => bar.protocol === protocol);
  const slots = (pick: (bar: LiquidityBar) => string | null): Slot[] =>
    slotsOf(
      period,
      asOf,
      mine.map((bar) => ({ from: bar.from, value: pick(bar) })),
    );
  const added = slots((bar) => bar.liquidity?.addedUsd ?? null);
  const removed = slots((bar) => bar.liquidity?.removedUsd ?? null);
  return added.map((slot, i) => ({
    from: slot.from,
    to: slot.to,
    added: slot.value,
    removed: removed[i]?.value ?? null,
  }));
}

function round(value: number): number {
  return Math.round(value * 10) / 10;
}
