/** An RFC3339 timestamp as a localized day, `"22 May 2026"`, with no time. */

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