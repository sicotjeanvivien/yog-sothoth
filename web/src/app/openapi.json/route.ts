/** The API contract the developers page reads, to download: the committed file, unchanged. */

import { openApiBody } from "@/lib/developers/contract-file";

export const dynamic = "force-static";

export function GET(): Response {
  return new Response(openApiBody(), {
    headers: { "Content-Type": "application/json; charset=utf-8" },
  });
}
