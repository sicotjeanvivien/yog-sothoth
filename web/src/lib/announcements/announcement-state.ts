/**
 * Announcement dismiss state — shared vocabulary.
 *
 * Plain module (no "use client") so both sides of the boundary can
 * import it: the dashboard layout (Server Component) reads the cookie
 * to know which announcements to skip on first paint — no flash, the
 * `sidebar-state` pattern — and the banner (client) rewrites it on
 * dismiss.
 *
 * One cookie holds every dismissed id as CSV, rather than one cookie
 * per announcement: fewer cookies to expire, trivial to parse, and the
 * list is capped so the header can never grow unbounded.
 */

import type { AnnouncementResponse } from "@/lib/api/schema/announcement";

export const ANNOUNCEMENTS_DISMISSED_COOKIE = "yog_announcements_dismissed";

/**
 * 90 days — long enough that a dismissed announcement stays gone for
 * its whole realistic display window, without the cookie living
 * forever.
 */
export const ANNOUNCEMENTS_COOKIE_MAX_AGE_S = 60 * 60 * 24 * 90;

/**
 * Upper bound on remembered ids. Oldest ids are dropped first — by
 * then their announcements' windows are long closed, so "forgetting"
 * them re-displays nothing.
 */
export const DISMISSED_IDS_MAX = 20;

/** Parse the CSV cookie value; garbage in → empty out, never a throw. */
export function parseDismissedIds(raw: string | undefined): number[] {
  if (!raw) return [];
  const ids = raw
    .split(",")
    .map((part) => Number(part))
    .filter((id) => Number.isInteger(id) && id > 0);
  return [...new Set(ids)];
}

/** Serialize back to CSV, deduplicated and capped (newest kept). */
export function serializeDismissedIds(ids: number[]): string {
  return [...new Set(ids)].slice(-DISMISSED_IDS_MAX).join(",");
}

/**
 * The one announcement the banner shows: the first non-dismissed
 * entry. The API already orders the active set most severe first then
 * most recent — the client deliberately re-derives nothing.
 */
export function pickAnnouncement(
  announcements: AnnouncementResponse[],
  dismissedIds: number[],
): AnnouncementResponse | null {
  const dismissed = new Set(dismissedIds);
  return announcements.find((a) => !dismissed.has(a.id)) ?? null;
}
