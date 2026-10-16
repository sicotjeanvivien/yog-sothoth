/**
 * The dismissed announcements, one capped CSV cookie. ⚠️ No "use client": the server layout
 * reads it so the first paint skips them, and the client banner rewrites it.
 */

import type { AnnouncementResponse } from "@/lib/api/schema/zod.gen";

export const ANNOUNCEMENTS_DISMISSED_COOKIE = "yog_announcements_dismissed";

export const ANNOUNCEMENTS_COOKIE_MAX_AGE_S = 60 * 60 * 24 * 90;

/** The oldest ids go first: their windows are long closed by then. */
export const DISMISSED_IDS_MAX = 20;

/** Garbage in, empty out: never a throw. */
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

/** The first non-dismissed one, in the API's order. */
export function pickAnnouncement(
  announcements: AnnouncementResponse[],
  dismissedIds: number[],
): AnnouncementResponse | null {
  const dismissed = new Set(dismissedIds);
  return announcements.find((a) => !dismissed.has(a.id)) ?? null;
}
