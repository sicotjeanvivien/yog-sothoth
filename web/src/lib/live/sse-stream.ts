/**
 * One live stream read with `fetch`, its ticket in the `Authorization` header: the transport of
 * both the dashboard's live connection and the developers explorer.
 */

import { createSseParser, type SseEvent } from "./sse-parser";

/** How a stream ended: never opened (`refused`, `unreachable`), or opened and then lost. */
export type SseEnd =
  | {
      kind: "refused";
      status: number;
      statusText: string;
      body: string;
      retryAfterMs: number | null;
    }
  | { kind: "unreachable"; error: unknown }
  | { kind: "closed" };

/** Resolves once the stream is over, `signal` aborting it included; never rejects. */
export async function openSseStream(
  url: string,
  ticket: string,
  handlers: { onOpen: () => void; onEvent: (event: SseEvent) => void },
  signal: AbortSignal,
): Promise<SseEnd> {
  let response: Response;
  try {
    response = await fetch(url, {
      headers: { Accept: "text/event-stream", Authorization: `Bearer ${ticket}` },
      cache: "no-store",
      signal,
    });
  } catch (error) {
    return { kind: "unreachable", error };
  }

  // ⚠️ A 200 that is no stream (a proxy's page) is a refusal: read as open, it resets the backoff.
  const isStream = response.headers.get("Content-Type")?.startsWith("text/event-stream") ?? false;
  if (response.status !== 200 || !isStream || response.body === null) {
    return {
      kind: "refused",
      status: response.status,
      statusText: response.statusText,
      body: await response.text().catch(() => ""),
      retryAfterMs: retryAfterMs(response.headers.get("Retry-After")),
    };
  }

  handlers.onOpen();
  const parser = createSseParser(handlers.onEvent);
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      parser.push(decoder.decode(value, { stream: true }));
    }
  } catch {
    // A network cut or the abort: the stream was open, so it ends the same way.
  }
  return { kind: "closed" };
}

/** `Retry-After` in seconds, the form the API sends; null when absent or another form. */
function retryAfterMs(header: string | null): number | null {
  if (header === null || !/^\d+$/.test(header.trim())) return null;
  return Number(header.trim()) * 1000;
}
