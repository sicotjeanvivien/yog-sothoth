/**
 * What the Overview may show, read on the span each measure carries: a period, a variation
 * on the previous window. ⚠️ A figure the facts do not cover whole
 * is "—", never a smaller number.
 */

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

/** Whether the `offset` read ends where the current one starts: a tranche or a freeze between the two reads moves one. */
export function previousWindowAdjoins(previous: MeasuredSpan, current: MeasuredSpan): boolean {
  return (
    previous.asOf !== null &&
    current.from !== null &&
    Date.parse(previous.asOf) === Date.parse(current.from)
  );
}
