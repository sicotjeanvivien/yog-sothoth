/**
 * What the Overview may show, read on the span each measure carries: a period, a variation
 * on the previous window, a variation of the TVL. ⚠️ A figure the facts do not cover whole
 * is "—", never a smaller number.
 */

import type { ProtocolResponse } from "@/lib/api/schema/zod.gen";

import { CLOSED_PERIODS, periodMs, type Period } from "./period";

/** The span every measure carries, `null` before the first hour is frozen. */
export type MeasuredSpan = {
  from: string | null;
  asOf: string | null;
  coveredSince: string | null;
};

/** `opensAt`: when the facts will cover the period, unknown before the first freeze. */
export type PeriodState = { open: true } | { open: false; opensAt: Date | null };

export function periodState(period: Period, span: MeasuredSpan): PeriodState {
  if (
    CLOSED_PERIODS.has(period) ||
    span.asOf === null ||
    span.coveredSince === null
  ) {
    return { open: false, opensAt: null };
  }
  const opensAt = new Date(Date.parse(span.coveredSince) + periodMs(period));
  return Date.parse(span.asOf) >= opensAt.getTime()
    ? { open: true }
    : { open: false, opensAt };
}

/** Whether the `offset` read covers its window whole: its total exists. */
export function previousWindowCovered(span: MeasuredSpan): boolean {
  return (
    span.from !== null &&
    span.coveredSince !== null &&
    Date.parse(span.from) >= Date.parse(span.coveredSince)
  );
}

type TvlPoint = { protocol: ProtocolResponse; at: string; tvlUsd: string | null };

/**
 * The protocol's TVL at the window's start, the first point of `/tvl/hourly`. `null` when
 * the TVL was first measured later, the hours before the floor being deleted.
 */
export function tvlStart<T extends TvlPoint>(
  points: readonly T[],
  from: string | null,
  protocol: ProtocolResponse,
): T | null {
  if (from === null) {
    return null;
  }
  const start = Date.parse(from);
  return (
    points.find((p) => p.protocol === protocol && Date.parse(p.at) === start) ??
    null
  );
}
