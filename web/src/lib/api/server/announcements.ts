/**
 * High-level fetcher for `GET /api/announcements/active`.
 *
 * No parameters — a thin wrapper over `apiGet` pinning the path and
 * the schema, like `fetchNetworkStatus`.
 *
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 *         The dashboard layout catches it: a broken announcement
 *         channel must never take the dashboard down.
 */

import { apiGet } from "../client/server";
import {
  AnnouncementListSchema,
  type AnnouncementResponse,
} from "../schema/announcement";

/** Fetch the currently-active operator announcements from `yog-api`. */
export async function fetchActiveAnnouncements(): Promise<
  AnnouncementResponse[]
> {
  return apiGet("/api/announcements/active", {}, AnnouncementListSchema);
}
