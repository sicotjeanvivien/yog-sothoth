/** Live data: the route itself is never cached. */
export const dynamic = "force-dynamic";

/**
 * The `(dashboard)` layout: the chrome is `DashboardShell`'s; this prepares only what the first
 * paint needs — the two frame cookies. ⚠️ No API call before rendering: the announcement
 * streams in its own `<Suspense>`.
 */

import { Suspense } from "react";
import { cookies } from "next/headers";

import { AnnouncementBanner } from "@/components/dashboard/announcements/announcement-banner";
import { PREFERENCES_OPEN_COOKIE } from "@/components/dashboard/sidebar/preferences-state";
import { SIDEBAR_COLLAPSED_COOKIE } from "@/components/dashboard/sidebar/sidebar-state";
import { DashboardShell } from "@/components/dashboard/shell/dashboard-shell";
import type { AnnouncementEntryResponse } from "@/lib/api/schema/zod.gen";
import { fetchOpenAnnouncements } from "@/lib/api/server/announcements";
import {
  ANNOUNCEMENTS_DISMISSED_COOKIE,
  parseDismissedIds,
  pickAnnouncement,
} from "@/lib/announcements/announcement-state";

/** Any failure is no banner: announcements must never take the dashboard down. */
async function safeOpenAnnouncements(): Promise<AnnouncementEntryResponse[]> {
  try {
    return await fetchOpenAnnouncements();
  } catch {
    return [];
  }
}

async function AnnouncementSlot() {
  const cookieStore = await cookies();
  const dismissedIds = parseDismissedIds(
    cookieStore.get(ANNOUNCEMENTS_DISMISSED_COOKIE)?.value,
  );
  const announcement = pickAnnouncement(
    await safeOpenAnnouncements(),
    dismissedIds,
  );
  if (!announcement) return null;
  return (
    <AnnouncementBanner announcement={announcement} dismissedIds={dismissedIds} />
  );
}

export default async function DashboardLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const cookieStore = await cookies();
  const initialCollapsed =
    cookieStore.get(SIDEBAR_COLLAPSED_COOKIE)?.value === "1";
  const initialPreferencesOpen =
    cookieStore.get(PREFERENCES_OPEN_COOKIE)?.value === "1";

  return (
    <DashboardShell
      initialCollapsed={initialCollapsed}
      initialPreferencesOpen={initialPreferencesOpen}
    >
      <Suspense fallback={null}>
        <AnnouncementSlot />
      </Suspense>
      {children}
    </DashboardShell>
  );
}
