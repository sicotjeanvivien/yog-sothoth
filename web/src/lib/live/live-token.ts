/**
 * A live token for the page's connection, asked of the dashboard's own server, which holds the key
 * (`app/api/live-token/route.ts`): the browser receives a token that opens that one stream once.
 */

import * as z from "zod";

import type { LiveTopic } from "./live-topics";

const zToken = z.object({ accessToken: z.string() });

/** @throws Error when the server gives no token: `openLive` then reopens later. */
export async function requestLiveToken(path: string, topics: readonly LiveTopic[]): Promise<string> {
  const response = await fetch("/api/live-token", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ path, topics }),
    cache: "no-store",
  });
  if (!response.ok) throw new Error(`no live token: ${response.status}`);
  return zToken.parse(await response.json()).accessToken;
}
