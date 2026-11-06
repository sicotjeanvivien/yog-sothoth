/**
 * `POST /api/live-token`: a live token for the stream the page is about to open, asked by the
 * server with the dashboard's key. A fixed path, not a Server Action, whose id each build changes:
 * a tab open across a deploy must still get the token it reopens with.
 */

import * as z from "zod";

import { ApiClientError } from "@/lib/api/errors";
import { apiPost } from "@/lib/api/client/server";
import { zLiveTokenResponse } from "@/lib/api/schema/zod.gen";

const zAsked = z.strictObject({ path: z.string(), topics: z.array(z.string()) });

export async function POST(request: Request): Promise<Response> {
  const asked = zAsked.safeParse(await request.json().catch(() => null));
  if (!asked.success) {
    return Response.json({ error: "a body { path, topics }" }, { status: 400 });
  }
  try {
    const token = await apiPost("/api/live-tokens", asked.data, zLiveTokenResponse);
    // ⚠️ The token alone: nothing else of the API's answer, and never the key, reaches the page.
    return Response.json(
      { accessToken: token.accessToken },
      { headers: { "Cache-Control": "no-store" } },
    );
  } catch (err) {
    const status =
      err instanceof ApiClientError && err.details.kind === "http" ? err.details.status : 502;
    return Response.json({ error: "no live token" }, { status });
  }
}
