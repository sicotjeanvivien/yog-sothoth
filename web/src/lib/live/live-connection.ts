/**
 * The page's one connection to `/api/live`, read as a `fetch` stream so that its live token goes
 * in `Authorization`, and reopened after a loss with a token of its own. ⚠️ Nothing is replayed:
 * every opening that follows a loss leaves a gap, and `onReopen` fires.
 */

import type { LiveTopic } from "./live-topics";
import { createSseParser } from "./sse-parser";

export type StreamStatus = "connecting" | "live" | "reconnecting";

export type LiveHandlers = {
  onStatus: (status: StreamStatus) => void;
  onEvent: (topic: LiveTopic, data: unknown) => void;
  onReopen: () => void;
};

/** A live token for this connection's path and topics: single-use, so one per opening. */
export type LiveTokenSource = () => Promise<string>;

const FIRST_BASE_MS = 10_000;
const MAX_DELAY_MS = 60_000;

/** Never before the API's `Retry-After`, which the caller adds. */
export function reopenDelayMs(refusals: number, random: number): number {
  const base = Math.min(FIRST_BASE_MS * 2 ** refusals, MAX_DELAY_MS);
  return base / 2 + (random * base) / 2;
}

/** The `Retry-After` of a refusal, in ms; 0 without one. */
function retryAfterMs(response: Response): number {
  const seconds = Number(response.headers.get("retry-after"));
  return Number.isFinite(seconds) && seconds > 0 ? seconds * 1000 : 0;
}

/** `afterLoss`: the page already missed events, so the first opening is a reopening. `close` tells the next. */
export function openLive(
  url: string,
  topics: readonly LiveTopic[],
  handlers: LiveHandlers,
  afterLoss: boolean,
  token: LiveTokenSource,
): { close: () => boolean } {
  const listened = new Set<string>(topics);
  let missed = afterLoss;
  let opened = false;
  let closed = false;
  let refusals = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let controller = new AbortController();

  function reopenAfter(atLeastMs: number): void {
    if (closed) return;
    handlers.onStatus("reconnecting");
    missed = true;
    const delay = Math.max(reopenDelayMs(refusals++, Math.random()), atLeastMs);
    timer = setTimeout(() => void connect(), delay);
  }

  function deliver(topic: string, raw: string): void {
    if (!listened.has(topic)) return;
    let data: unknown;
    try {
      data = JSON.parse(raw);
    } catch {
      console.warn(`live ${topic}: unparseable event skipped`);
      return;
    }
    handlers.onEvent(topic as LiveTopic, data);
  }

  async function connect(): Promise<void> {
    controller = new AbortController();
    let response: Response;
    try {
      const bearer = await token();
      response = await fetch(url, {
        headers: { Accept: "text/event-stream", Authorization: `Bearer ${bearer}` },
        signal: controller.signal,
        cache: "no-store",
      });
    } catch {
      reopenAfter(0);
      return;
    }
    if (!response.ok || response.body === null) {
      reopenAfter(retryAfterMs(response));
      return;
    }

    handlers.onStatus("live");
    if (missed) handlers.onReopen();
    missed = false;
    opened = true;
    refusals = 0;

    const parser = createSseParser((event) => deliver(event.event, event.data));
    const reader = response.body.pipeThrough(new TextDecoderStream()).getReader();
    try {
      for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        parser.push(value);
      }
    } catch {
      // A reset or an abort: either way the stream is over, and `close` says which.
    }
    reopenAfter(0);
  }

  void connect();

  return {
    close: () => {
      closed = true;
      clearTimeout(timer);
      controller.abort();
      return missed || opened;
    },
  };
}
