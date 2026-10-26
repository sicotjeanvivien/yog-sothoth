/**
 * The dismissed announcements, one capped CSV cookie. ⚠️ No "use client": the server layout
 * reads it so the first paint skips them, and the client banner rewrites it.
 */

import type {
  AnnouncementEntryResponse,
  AnnouncementSeverityResponse,
} from "@/lib/api/schema/zod.gen";

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

const SEVERITY_RANK: Record<AnnouncementSeverityResponse, number> = {
  critical: 0,
  warning: 1,
  info: 2,
};

/** The most severe non-dismissed one; at equal severity, the first in the API's order. */
export function pickAnnouncement(
  announcements: readonly AnnouncementEntryResponse[],
  dismissedIds: number[],
): AnnouncementEntryResponse | null {
  const dismissed = new Set(dismissedIds);
  const shown = announcements.filter((a) => !dismissed.has(a.id));
  // ⚠️ A stable sort: the API's latest-start order breaks the ties.
  shown.sort((a, b) => SEVERITY_RANK[a.severity] - SEVERITY_RANK[b.severity]);
  return shown[0] ?? null;
}
