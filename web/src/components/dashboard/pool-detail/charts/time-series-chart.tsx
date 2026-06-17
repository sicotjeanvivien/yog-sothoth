"use client";

/**
 * Minimal responsive time-series chart built on visx primitives (no
 * batteries-included charting lib — primitives we compose, so we're never
 * boxed in). Renders either an `area` or a `line` over a time x-axis, with a
 * value y-axis and a hover tooltip.
 *
 * Props are serializable only (it's a Client Component fed by a Server
 * Component): data points are `{ t: epoch ms, value }`, and formatting is
 * driven by a `valueFormat` flag rather than a function prop (functions can't
 * cross the RSC boundary).
 */

import { useCallback, useMemo } from "react";
import { AreaClosed, LinePath } from "@visx/shape";
import { scaleLinear, scaleTime } from "@visx/scale";
import { AxisBottom, AxisLeft } from "@visx/axis";
import { Group } from "@visx/group";
import { ParentSize } from "@visx/responsive";
import { useTooltip, TooltipWithBounds, defaultStyles } from "@visx/tooltip";

export type ChartPoint = { t: number; value: number };

type ValueFormat = "usd" | "bps";

type Props = {
  data: ChartPoint[];
  variant: "area" | "line";
  valueFormat: ValueFormat;
  color: string;
  locale: string;
  height?: number;
};

const MARGIN = { top: 8, right: 16, bottom: 24, left: 52 };

function formatValue(value: number, kind: ValueFormat, locale: string): string {
  if (kind === "bps") {
    // Effective rate in basis points; keep it terse.
    return `${Number(value.toFixed(2))} bps`;
  }
  // USD, compact ($1.2K, $3.4M) — these are per-hour amounts, often small.
  return new Intl.NumberFormat(locale, {
    style: "currency",
    currency: "USD",
    notation: "compact",
    maximumFractionDigits: 2,
  }).format(value);
}

function formatDate(t: number, locale: string): string {
  return new Intl.DateTimeFormat(locale, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
  }).format(new Date(t));
}

function Inner({
  width,
  height,
  data,
  variant,
  valueFormat,
  color,
  locale,
}: Props & { width: number; height: number }) {
  const innerW = Math.max(0, width - MARGIN.left - MARGIN.right);
  const innerH = Math.max(0, height - MARGIN.top - MARGIN.bottom);

  const xScale = useMemo(() => {
    const ts = data.map((d) => d.t);
    const min = Math.min(...ts);
    const max = Math.max(...ts);
    return scaleTime({
      domain: [new Date(min), new Date(max === min ? max + 1 : max)],
      range: [0, innerW],
    });
  }, [data, innerW]);

  const yScale = useMemo(() => {
    const maxV = Math.max(0, ...data.map((d) => d.value));
    return scaleLinear({
      domain: [0, maxV === 0 ? 1 : maxV * 1.1],
      range: [innerH, 0],
      nice: true,
    });
  }, [data, innerH]);

  const { showTooltip, hideTooltip, tooltipData, tooltipLeft, tooltipTop } =
    useTooltip<ChartPoint>();

  const handleMove = useCallback(
    (event: React.MouseEvent<SVGRectElement>) => {
      const first = data[0];
      if (!first) return;
      const rect = event.currentTarget.getBoundingClientRect();
      const x = event.clientX - rect.left;
      const t = xScale.invert(x).getTime();
      // Nearest point by time — small series, a linear scan is fine.
      let nearest: ChartPoint = first;
      for (const p of data) {
        if (Math.abs(p.t - t) < Math.abs(nearest.t - t)) nearest = p;
      }
      showTooltip({
        tooltipData: nearest,
        tooltipLeft: xScale(new Date(nearest.t)),
        tooltipTop: yScale(nearest.value),
      });
    },
    [data, xScale, yScale, showTooltip],
  );

  return (
    <div style={{ position: "relative" }}>
      <svg width={width} height={height}>
        <Group left={MARGIN.left} top={MARGIN.top}>
          {variant === "area" ? (
            <AreaClosed<ChartPoint>
              data={data}
              x={(d) => xScale(new Date(d.t))}
              y={(d) => yScale(d.value)}
              yScale={yScale}
              fill={color}
              fillOpacity={0.18}
              stroke={color}
              strokeWidth={1.5}
            />
          ) : (
            <LinePath<ChartPoint>
              data={data}
              x={(d) => xScale(new Date(d.t))}
              y={(d) => yScale(d.value)}
              stroke={color}
              strokeWidth={1.5}
            />
          )}

          <AxisLeft
            scale={yScale}
            numTicks={4}
            tickFormat={(v) => formatValue(Number(v), valueFormat, locale)}
            stroke="#475569"
            tickStroke="#475569"
            tickLabelProps={() => ({
              fill: "#94a3b8",
              fontSize: 10,
              textAnchor: "end",
              dx: -4,
              dy: 3,
            })}
          />
          <AxisBottom
            top={innerH}
            scale={xScale}
            numTicks={5}
            tickFormat={(v) => formatDate(Number(v), locale)}
            stroke="#475569"
            tickStroke="#475569"
            tickLabelProps={() => ({
              fill: "#94a3b8",
              fontSize: 10,
              textAnchor: "middle",
            })}
          />

          {tooltipData && (
            <circle
              cx={tooltipLeft}
              cy={tooltipTop}
              r={3.5}
              fill={color}
              stroke="#0b1120"
              strokeWidth={1.5}
              pointerEvents="none"
            />
          )}

          {/* Transparent capture surface for hover. */}
          <rect
            width={innerW}
            height={innerH}
            fill="transparent"
            onMouseMove={handleMove}
            onMouseLeave={hideTooltip}
          />
        </Group>
      </svg>

      {tooltipData && (
        <TooltipWithBounds
          left={(tooltipLeft ?? 0) + MARGIN.left}
          top={(tooltipTop ?? 0) + MARGIN.top}
          style={{
            ...defaultStyles,
            background: "#0b1120",
            border: "1px solid #1e293b",
            color: "#e2e8f0",
            fontSize: 11,
          }}
        >
          <div style={{ fontWeight: 600 }}>
            {formatValue(tooltipData.value, valueFormat, locale)}
          </div>
          <div style={{ color: "#94a3b8" }}>
            {formatDate(tooltipData.t, locale)}
          </div>
        </TooltipWithBounds>
      )}
    </div>
  );
}

export function TimeSeriesChart(props: Props) {
  const height = props.height ?? 220;
  if (props.data.length === 0) return null;
  return (
    <div style={{ width: "100%", height }}>
      <ParentSize>
        {({ width }) =>
          width > 0 ? <Inner {...props} width={width} height={height} /> : null
        }
      </ParentSize>
    </div>
  );
}
