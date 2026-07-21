/**
 * Format an RFC3339 timestamp as a relative time string, e.g.
 * "2 minutes ago" / "il y a 2 minutes".
 *
 * Uses `Intl.RelativeTimeFormat` to get the locale-aware wording;
 * the unit (seconds / minutes / hours / days / months / years) is
 * chosen based on the absolute distance to the reference point,
 * defaulting to "now".
 *
 * The reference point defaults to `Date.now()` but can be injected
 * for tests or for consistent rendering across a server response.
 *
 * Locale defaults to the active next-intl locale at the call site,
 * but is passed in explicitly to keep the function pure and easy
 * to unit-test.
 *
 * `style` maps straight to `Intl.RelativeTimeFormat`'s option:
 * `"long"` (default) → "2 hours ago" / "il y a 2 heures"; `"short"` →
 * the compact, locale-aware form ("2 hr. ago" / "il y a 2 h") used in
 * dense tables. Both stay fully localized — no manual abbreviation.
 * (`"narrow"` is avoided: in French it drops "il y a" for a bare "-2 h".)
 */

const THRESHOLDS: ReadonlyArray<{ unit: Intl.RelativeTimeFormatUnit; seconds: number }> = [
  { unit: "year", seconds: 60 * 60 * 24 * 365 },
  { unit: "month", seconds: 60 * 60 * 24 * 30 },
  { unit: "day", seconds: 60 * 60 * 24 },
  { unit: "hour", seconds: 60 * 60 },
  { unit: "minute", seconds: 60 },
  { unit: "second", seconds: 1 },
];

export function formatRelativeTime(
  isoTimestamp: string,
  locale: string,
  options: { now?: Date; style?: Intl.RelativeTimeFormatStyle } = {},
): string {
  const { now = new Date(), style = "long" } = options;

  const then = new Date(isoTimestamp);
  if (Number.isNaN(then.getTime())) {
    return "—";
  }

  const diffSeconds = Math.round((then.getTime() - now.getTime()) / 1000);
  const absSeconds = Math.abs(diffSeconds);

  const formatter = new Intl.RelativeTimeFormat(locale, {
    numeric: "auto",
    style,
  });

  // Pick the largest unit that still produces a value >= 1.
  for (const { unit, seconds } of THRESHOLDS) {
    if (absSeconds >= seconds) {
      const value = Math.round(diffSeconds / seconds);
      return formatter.format(value, unit);
    }
  }

  // Fallback: less than a second in either direction.
  return formatter.format(0, "second");
}