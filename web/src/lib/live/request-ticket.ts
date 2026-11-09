"use server";

/**
 * The page's ticket to the live stream: the dashboard's key, which stays on the server, traded
 * for a single-use ticket. Not a BFF: no API answer goes through Next, only tickets.
 */

import { apiPost } from "@/lib/api/client/server";
import { zTicketResponse } from "@/lib/api/schema/zod.gen";

/** @throws ApiClientError when the API refuses the key or cannot be reached. */
export async function requestTicket(): Promise<string> {
  const { ticket } = await apiPost("/api/auth/tickets", zTicketResponse);
  return ticket;
}
