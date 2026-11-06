"use server";

/**
 * A live token for the page's connection, asked by the dashboard's server with its key: the key
 * stays on the server, the browser receives a token that opens that one stream once.
 */

import { apiPost } from "@/lib/api/client/server";
import { zLiveTokenResponse } from "@/lib/api/schema/zod.gen";

import type { LiveTopic } from "./live-topics";

export async function requestLiveToken(path: string, topics: readonly LiveTopic[]): Promise<string> {
  const token = await apiPost("/api/live-tokens", { path, topics }, zLiveTokenResponse);
  return token.accessToken;
}
