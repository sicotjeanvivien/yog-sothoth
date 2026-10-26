/**
 * `GET /api/announcements?status=open`, its first page.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import { apiGet } from "../client/server";
import {
  zListAnnouncementsResponse,
  type AnnouncementEntryResponse,
} from "../schema/zod.gen";

export async function fetchOpenAnnouncements(): Promise<AnnouncementEntryResponse[]> {
  // ⚠️ The widest page: it is ordered by start, the banner by severity.
  const page = await apiGet(
    "/api/announcements",
    { status: "open", limit: 200 },
    zListAnnouncementsResponse,
  );
  return page.items;
}
