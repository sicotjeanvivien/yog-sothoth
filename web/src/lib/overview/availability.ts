/**
 * Whether the Overview may show a variation on the previous window, read on the span each measure
 * carries. ⚠️ A figure the facts do not cover whole is "—", never a smaller number.
 */

/** The span every measure carries, `null` before the first hour is frozen. */
export type MeasuredSpan = {
  from: string | null;
  asOf: string | null;
  coveredSince: string | null;
};

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
