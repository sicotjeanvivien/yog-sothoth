/**
 * Force dynamic execution — the response depends on live data from
 * yog-api, never cache the route itself. Caching strategy lives in
 * `apiGet` (currently `no-store` for every upstream call).
 */
export const dynamic = "force-dynamic";

/**
 * Layout for the dashboard route group `(dashboard)`.
 *
 * Delegates the whole responsive chrome — sidebar, mobile drawer,
 * header, overlay — to `DashboardShell`. The route group does not
 * appear in the URL, so pages keep their natural paths
 * (`/[locale]/overview`, `/[locale]/pools`, …) while sharing this
 * chrome.
 *
 * This file is intentionally thin: it is a Server Component and does
 * nothing but read the sidebar-collapse cookie (so the first paint
 * already has the user's preferred width — no flash) and mount the
 * (client) shell. All interactivity and layout mechanics live in
 * `DashboardShell`.
 */

import { cookies } from "next/headers";

import { AnnouncementBanner } from "@/components/dashboard/announcements/announcement-banner";
import { SIDEBAR_COLLAPSED_COOKIE } from "@/components/dashboard/sidebar/sidebar-state";
import { DashboardShell } from "@/components/dashboard/shell/dashboard-shell";
import type { AnnouncementResponse } from "@/lib/api/schema/announcement";
import { fetchActiveAnnouncements } from "@/lib/api/server/announcements";
import {
  ANNOUNCEMENTS_DISMISSED_COOKIE,
  parseDismissedIds,
  pickAnnouncement,
} from "@/lib/announcements/announcement-state";

/**
 * Best-effort read of the active announcements: a broken announcement
 * channel (API down, schema drift) must never take the dashboard down,
 * so any failure degrades to "no banner".
 */
async function safeActiveAnnouncements(): Promise<AnnouncementResponse[]> {
  try {
    return await fetchActiveAnnouncements();
  } catch {
    return [];
  }
}

export default async function DashboardLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const cookieStore = await cookies();
  const initialCollapsed =
    cookieStore.get(SIDEBAR_COLLAPSED_COOKIE)?.value === "1";

  const dismissedIds = parseDismissedIds(
    cookieStore.get(ANNOUNCEMENTS_DISMISSED_COOKIE)?.value,
  );
  const announcement = pickAnnouncement(
    await safeActiveAnnouncements(),
    dismissedIds,
  );

  return (
    <DashboardShell initialCollapsed={initialCollapsed}>
      {announcement && (
        <AnnouncementBanner
          announcement={announcement}
          dismissedIds={dismissedIds}
        />
      )}
      {children}
    </DashboardShell>
  );
}
