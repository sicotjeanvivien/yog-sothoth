/**
 * Format an RFC3339 timestamp as a locale-aware absolute date,
 * e.g.
 *   "2026-05-22T05:34:30.141197Z"  →  "22 May 2026"  (en)
 *                                  →  "22 mai 2026"  (fr)
 *
 * Uses `Intl.DateTimeFormat` with the `long` month style. No time
 * component — this helper targets "first seen / created on"-style
 * fields where the day is the meaningful unit.
 *
 * For relative durations like "2 min ago", use `formatRelativeTime`
 * instead.
 */

export function formatAbsoluteDate(
  isoTimestamp: string,
  locale: string,
): string {
  const date = new Date(isoTimestamp);
  if (Number.isNaN(date.getTime())) {
    return "—";
  }

  return new Intl.DateTimeFormat(locale, {
    day: "numeric",
    month: "long",
    year: "numeric",
  }).format(date);
}