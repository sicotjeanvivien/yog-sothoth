/**
 * Date display helpers.
 *
 * Two formatters cover the dashboard's needs:
 *
 *   - `formatRelative`: human-friendly "3 minutes ago" for recent
 *     events. Locale-aware via the standard `Intl.RelativeTimeFormat`.
 *   - `formatAbsolute`: deterministic "2026-05-12 03:18 UTC" for
 *     timestamps shown in table cells or tooltips. Locale-independent
 *     output (intentional — pubkey-style precision).
 *
 * Both accept either an ISO-8601 string (yog-api wire format) or a
 * `Date` instance, and return null for invalid inputs so the React
 * layer can branch without try/catch.
 */

/** Locale tag passed to Intl APIs. Matches the next-intl supported locales. */
export type FormatLocale = "en" | "fr";

/**
 * Parse the input into a `Date` once, returning null on invalid input
 * so the rest of the formatter chain can branch on a single check.
 */
function toDate(input: string | Date): Date | null {
  const date = input instanceof Date ? input : new Date(input);
  return Number.isNaN(date.getTime()) ? null : date;
}

/**
 * Thresholds used by `formatRelative` to pick a unit. Order matters:
 * the first threshold whose absolute delta exceeds the value is used.
 *
 * Calibration choices:
 *   - Anything under a minute is rounded to "just now" — avoids
 *     flickering "1 second ago / 2 seconds ago" updates.
 *   - "1 day" kicks in at 24h, "1 month" at 30d (Intl default).
 */
const RELATIVE_UNITS: Array<{ unit: Intl.RelativeTimeFormatUnit; seconds: number }> = [
  { unit: "year", seconds: 60 * 60 * 24 * 365 },
  { unit: "month", seconds: 60 * 60 * 24 * 30 },
  { unit: "day", seconds: 60 * 60 * 24 },
  { unit: "hour", seconds: 60 * 60 },
  { unit: "minute", seconds: 60 },
];

/**
 * Format a timestamp as a human-friendly relative phrase.
 *
 * @param input  ISO-8601 string from `yog-api`, or a `Date`.
 * @param locale Active locale, controls phrasing ("3 minutes ago" / "il y a 3 minutes").
 * @param now    Reference point. Defaulting to `new Date()` makes the
 *               function impure but matches typical UI usage; for tests,
 *               pass a deterministic `Date`.
 *
 * Returns null for malformed input. Returns "just now" for any delta
 * shorter than one minute.
 */
export function formatRelative(
  input: string | Date,
  locale: FormatLocale,
  now: Date = new Date(),
): string | null {
  const target = toDate(input);
  if (target === null) {
    return null;
  }

  const deltaSeconds = (target.getTime() - now.getTime()) / 1000;
  const absDelta = Math.abs(deltaSeconds);

  // Below one minute: collapse to a fixed phrase per locale. The
  // standard `Intl.RelativeTimeFormat` would otherwise yield "0 minutes
  // ago" which reads worse than a dedicated label.
  if (absDelta < 60) {
    return locale === "fr" ? "à l'instant" : "just now";
  }

  const formatter = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });

  for (const { unit, seconds } of RELATIVE_UNITS) {
    if (absDelta >= seconds) {
      const value = Math.round(deltaSeconds / seconds);
      return formatter.format(value, unit);
    }
  }

  // Theoretically unreachable: a delta >= 60s but smaller than every
  // unit's threshold cannot exist (the smallest unit is 60s). Kept as
  // a defensive fallback.
  return formatter.format(Math.round(deltaSeconds / 60), "minute");
}

/**
 * Format a timestamp as a deterministic UTC string suitable for table
 * cells: `YYYY-MM-DD HH:mm UTC`. Locale-independent on purpose — the
 * absolute date is metadata, not narrative content.
 *
 * Returns null for malformed input.
 */
export function formatAbsolute(input: string | Date): string | null {
  const date = toDate(input);
  if (date === null) {
    return null;
  }

  const year = date.getUTCFullYear();
  const month = String(date.getUTCMonth() + 1).padStart(2, "0");
  const day = String(date.getUTCDate()).padStart(2, "0");
  const hours = String(date.getUTCHours()).padStart(2, "0");
  const minutes = String(date.getUTCMinutes()).padStart(2, "0");

  return `${year}-${month}-${day} ${hours}:${minutes} UTC`;
}