/**
 * An RFC3339 timestamp as "2 minutes ago", locale and reference point passed in. `style`
 * `"short"` is the compact localized form; ⚠️ not `"narrow"`, which in French drops "il y a".
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

  for (const { unit, seconds } of THRESHOLDS) {
    if (absSeconds >= seconds) {
      const value = Math.round(diffSeconds / seconds);
      return formatter.format(value, unit);
    }
  }

  // Fallback: less than a second in either direction.
  return formatter.format(0, "second");
}