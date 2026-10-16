/**
 * Dates from yog-api's ISO-8601 strings or a `Date`: `formatRelative` is locale-aware,
 * `formatAbsolute` deliberately not. Both return null on invalid input.
 */

/** The next-intl locales. */
export type FormatLocale = "en" | "fr";

function toDate(input: string | Date): Date | null {
  const date = input instanceof Date ? input : new Date(input);
  return Number.isNaN(date.getTime()) ? null : date;
}

/** Largest first: the first unit the delta reaches is used. */
const RELATIVE_UNITS: Array<{ unit: Intl.RelativeTimeFormatUnit; seconds: number }> = [
  { unit: "year", seconds: 60 * 60 * 24 * 365 },
  { unit: "month", seconds: 60 * 60 * 24 * 30 },
  { unit: "day", seconds: 60 * 60 * 24 },
  { unit: "hour", seconds: 60 * 60 },
  { unit: "minute", seconds: 60 },
];

/**
 * "3 minutes ago", or a fixed "just now" under a minute. Pass `now` for a deterministic test.
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

  // Unreachable: the smallest unit is 60 s.
  return formatter.format(Math.round(deltaSeconds / 60), "minute");
}

/** `YYYY-MM-DD HH:mm UTC`, locale-independent on purpose. */
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