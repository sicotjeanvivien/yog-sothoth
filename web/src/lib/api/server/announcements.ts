/**
 * `GET /api/announcements/active`.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import { apiGet } from "../client/server";
import {
  zListActiveAnnouncementsResponse,
  type AnnouncementResponse,
} from "../schema/zod.gen";

export async function fetchActiveAnnouncements(): Promise<
  AnnouncementResponse[]
> {
  return apiGet("/api/announcements/active", {}, zListActiveAnnouncementsResponse);
}
